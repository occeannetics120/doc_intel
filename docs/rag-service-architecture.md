# RAG Service Architecture — fit_voyage

## Overview

A Rust-based RAG (Retrieval-Augmented Generation) service that lets users upload maritime documents and ask questions grounded in their content. Vessel particulars (engine specs, dimensions) from the existing `VesselParticularsEntity` are injected as structured context into every prompt — no embedding needed for structured data. Unstructured documents (incident reports, manuals, certificates, voyage reports) go through the full chunk → embed → Qdrant pipeline.

---

## System Topology

```
fit_voyage_offshore (Next.js)
  └─ src/lib/services/rag/rag.service.ts          new
  └─ src/app/(dashboard)/documents/page.tsx       new
        │
        ▼ POST /api/ai/rag/*
fit_voyage_backend_nest (NestJS)
  └─ src/application/ai-analytics/
       controllers/rag-proxy.controller.ts        new
       services/rag-proxy.service.ts              new
       dto/rag.dto.ts                             new
        │ fetches VesselParticularsEntity → passes as vessel_context
        ▼ POST http://localhost:8090/*
fit_voyage_rag_service/ (Rust — Axum)             new service
  └─ POST /documents    → ingest pipeline
  └─ POST /search       → vector retrieval
  └─ POST /ask          → RAG (retrieval + LLM)
        │                      │
   Qdrant :6333          OpenAI API
```

---

## Rust Service Directory Structure

```
fit_voyage_rag_service/
├── Cargo.toml
├── .env
└── src/
    ├── main.rs
    ├── config.rs
    ├── routes/
    │   ├── mod.rs
    │   ├── documents.rs      POST /documents — ingest entry point
    │   ├── search.rs         POST /search — vector retrieval
    │   └── ask.rs            POST /ask — full RAG
    ├── pipeline/
    │   ├── mod.rs
    │   ├── extractor.rs      PDF (lopdf) / DOCX (docx-rs) / TXT → raw String
    │   ├── chunker.rs        500-word chunks, 50-word overlap
    │   └── embedder.rs       OpenAI text-embedding-3-small (1536 dims)
    ├── store/
    │   ├── mod.rs
    │   └── qdrant.rs         upsert + search, compound OR filter by vessel_id / vessel_type / design_type / fleet
    └── llm/
        ├── mod.rs
        └── openai.rs         build prompt, call gpt-4o-mini, return answer + citations
```

### Cargo.toml Dependencies

```toml
[dependencies]
axum               = "0.7"
tokio              = { version = "1", features = ["full"] }
reqwest            = { version = "0.12", features = ["json", "multipart"] }
serde              = { version = "1", features = ["derive"] }
serde_json         = "1"
uuid               = { version = "1", features = ["v4"] }
lopdf              = "0.32"
docx-rs            = "0.4"
qdrant-client      = "1"
anyhow             = "1"
tracing            = "0.1"
tracing-subscriber = "0.3"
dotenvy            = "0.15"
```

---

## Ingestion Pipeline

Every uploaded document flows through this chain:

```
File bytes (PDF / DOCX / TXT)
        │
        ▼ extractor.rs
  match mime_type {
    application/pdf  → lopdf → raw String
    application/docx → docx-rs → raw String
    text/plain       → direct String
  }
        │
        ▼ chunker.rs
  500-word chunks, 50-word overlap
  Chunk { id: uuid, text: String, metadata: { vessel_id, doc_name, doc_type, chunk_index, doc_scope, scope_id } }
        │
        ▼ embedder.rs
  POST https://api.openai.com/v1/embeddings
  model: "text-embedding-3-small"
  → Vec<Vec<f32>>  (1536 dims per chunk)
        │
        ▼ qdrant.rs
  collection: "maritime_docs"
  PointStruct { id: chunk.id, vector: embedding, payload: { text, vessel_id, doc_name, doc_type, chunk_index, doc_scope, scope_id } }

  doc_scope values:
    "vessel"      — only this vessel (scope_id = vessel_id)
    "vessel_type" — all vessels of this engine/hull type (scope_id = type code e.g. "MAN-6L28")
    "design_type" — all vessels of this design class (scope_id = design class code)
    "fleet"       — all vessels (scope_id = null)
```

---

## Retrieval + RAG Flow

```
POST /ask  { question, vessel_id, vessel_context }
        │
        ▼ embed the question (embedder.rs)
  → Vec<f32>
        │
        ▼ qdrant.rs
  search "maritime_docs", top_k: 5, compound OR filter:
    (doc_scope == "vessel"      AND vessel_id    == requested_vessel_id)
    (doc_scope == "vessel_type" AND scope_id     == requested_vessel_type)
    (doc_scope == "design_type" AND scope_id     == requested_design_type)
    (doc_scope == "fleet")
  → Vec<ScoredPoint> with payload.text

  vessel_type and design_type are resolved by NestJS from VesselParticularsEntity
  before forwarding the request to the Rust service.
        │
        ▼ openai.rs — build prompt:

  system: "You are a maritime technical analyst for vessel {vessel_name}.
           Answer only from the provided context."

  user:
    === VESSEL PARTICULARS ===
    {vessel_context as JSON}

    === DOCUMENT CONTEXT ===
    [chunk 1 text]
    ---
    [chunk 2 text]
    ---
    ...

    === QUESTION ===
    {question}

        │
        ▼ POST https://api.openai.com/v1/chat/completions
  model: gpt-4o-mini
        │
        ▼ return { answer, source_chunks: [{ text, doc_name, doc_type, chunk_index }] }
```

`source_chunks` in the response are the citations — this is what makes answers grounded, not hallucinated.

---

## API Contracts

### POST /documents
Multipart form: `file` (bytes) + `vessel_id` + `doc_type` + `doc_name` + `doc_scope` + `scope_id`

- `doc_scope`: `"vessel"` | `"vessel_type"` | `"design_type"` | `"fleet"`
- `scope_id`: the vessel_id, type code, or design class code — omit for `"fleet"`

Response:
```json
{ "chunk_count": 12, "collection": "maritime_docs" }
```

### POST /search
```json
{
  "query": "fuel leakage near UAE",
  "vessel_id": 1234567,
  "vessel_type": "MAN-6L28",
  "design_type": "design-class-A",
  "top_k": 5
}
```
Response:
```json
{
  "chunks": [
    { "text": "...", "doc_name": "incident_2024_q3.pdf", "doc_type": "incident_report", "chunk_index": 3, "score": 0.91, "doc_scope": "vessel_type" }
  ]
}
```

### POST /ask
```json
{
  "question": "What recurring engine issues exist?",
  "vessel_id": 1234567,
  "vessel_type": "MAN-6L28",
  "design_type": "design-class-A",
  "vessel_context": {
    "vessel_name": "MV Example",
    "main_engines": [{ "model": "MAN 6L28/32H", "mcr_power": 1980, "stroke_type": 4 }],
    "auxiliary_engines": [...],
    "deadweight": 3200,
    "length_overall": 78.5
  }
}
```
Response:
```json
{
  "answer": "Based on incident reports from Q3 2024, engine 2 showed...",
  "source_chunks": [
    { "text": "...", "doc_name": "incident_2024_q3.pdf", "chunk_index": 3 }
  ]
}
```

---

## NestJS Layer

### Files to Create

| File | Purpose |
|------|---------|
| `src/application/ai-analytics/controllers/rag-proxy.controller.ts` | Routes: POST /ai/rag/documents, /search, /ask |
| `src/application/ai-analytics/services/rag-proxy.service.ts` | HttpService to Rust, fetches VesselParticularsEntity |
| `src/application/ai-analytics/dto/rag.dto.ts` | Request/response DTOs with Swagger decorators |

### Files to Modify

| File | Change |
|------|--------|
| `src/application/ai-analytics/ai-analytics.module.ts` | Register RagProxyController + RagProxyService |
| `.env` | Add `RAG_SERVICE_URL=http://localhost:8090` |

### RagProxyService Key Methods

```typescript
// Fetches vessel particulars from DB, resolves vessel_type + design_type, builds vessel_context, POSTs to Rust
ask(question: string, vesselId: number): Promise<RagAskResponse>

// Multipart POST to Rust /documents — doc_scope + scope_id passed through from request body
ingestDocument(file: Express.Multer.File, vesselId: number, docType: string, docScope: string, scopeId?: string): Promise<void>

// Fetches vessel_type + design_type from VesselParticularsEntity, then POSTs to Rust /search
search(query: string, vesselId: number): Promise<RagSearchResponse>
```

`vessel_type` and `design_type` are derived from `VesselParticularsEntity` (already fetched for `vessel_context`) — the Rust service never touches the DB directly.

Follows the exact same `HttpService` + `ConfigService` + `firstValueFrom` pattern as `ai-analytics.service.ts`.

---

## Frontend Layer

### Files to Create

| File | Purpose |
|------|---------|
| `src/lib/services/rag/rag.service.ts` | API calls via swagger-generated `api` client |
| `src/app/(dashboard)/documents/page.tsx` | File upload UI + question/answer chat UI |

### Env Var (add to existing `.env.local`)

```
# already covered by NEXT_PUBLIC_API_URL
```

---

## Existing Code to Reuse

| What | File |
|------|------|
| VesselParticularsEntity | `src/domain/vessel/entities/vessel-particulars.entity.ts` |
| VesselService.findByVesselId() | `src/domain/vessel/services/vessel.service.ts` |
| HttpService pattern | `src/application/ai-analytics/services/ai-analytics.service.ts` |
| JwtAuthGuard + ClientScopeGuard | `src/common/guards/` |
| ApiResponseDto | `src/common/dto/api-response.dto.ts` |
| Frontend service pattern | `src/lib/services/users/users.service.ts` |

---

## Environment Variables

**`fit_voyage_rag_service/.env`**
```
OPENAI_API_KEY=sk-...
QDRANT_URL=http://localhost:6333
QDRANT_API_KEY=
RUST_LOG=info
PORT=8090
```

**`fit_voyage_backend_nest/.env` (add)**
```
RAG_SERVICE_URL=http://localhost:8090
```

---

## Build Sequence

| Step | Task | Verify |
|------|------|--------|
| 1 | Rust scaffold — Axum server on :8090 | `GET /health` → 200 |
| 2 | `extractor.rs` — upload PDF, get raw text | POST /documents returns extracted text in debug mode |
| 3 | `chunker.rs` — text → Vec<Chunk> | unit test: chunk count correct, overlap present |
| 4 | Qdrant via Docker — create `maritime_docs` collection | Qdrant dashboard at :6333 shows collection |
| 5 | `embedder.rs` — call OpenAI embeddings | assert `embedding.len() == 1536` |
| 6 | `qdrant.rs` — upsert + search | `POST /search` with known phrase returns matching chunk |
| 7 | `openai.rs` — full RAG prompt + GPT call | `POST /ask` returns grounded answer + source_chunks |
| 8 | NestJS proxy wired into AiAnalyticsModule | `POST /api/ai/rag/ask` passes through correctly |
| 9 | Frontend upload + chat UI | Upload PDF → ask question → see answer with citations |

---

## MVP Milestone (Step 7)

Upload a real maritime incident report PDF for a vessel.  
Call `POST /ask { "question": "What happened?", "vessel_id": ... }`.  
Receive a grounded answer with `source_chunks` naming the document and chunk index.

That is the complete RAG loop.

---

## Deferred (Phase 2+)

| Feature | Why deferred |
|---------|-------------|
| Local LLM (Ollama) | CUDA/VRAM rabbit hole; OpenAI is $0.001/call |
| Re-ranking (cross-encoders) | Phase 2 retrieval quality optimization |
| Streaming responses | Nice UX, not required for correctness |
| LangChain/LlamaIndex Rust crates | Learn primitives first, frameworks later |
| pgvector instead of Qdrant | Qdrant has better Rust client + vessel_id filter support |
