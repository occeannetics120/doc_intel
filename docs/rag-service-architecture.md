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
fit_voyage_rag_service/ (Rust — Actix)             new service
  └─ POST /documents    → ingest pipeline
  └─ POST /search       → hybrid vector retrieval (dense + sparse)
  └─ POST /ask          → RAG (retrieval + LLM)
        │                      │                    │
   Qdrant :6333          Ollama :11434          SPLADE :8091
                      (gte-Qwen2-7B, local)    (FastAPI, sparse vectors)
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
    │   └── embedder.rs       Ollama gte-Qwen2-7B (3584 dims, local)
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

## SPLADE FastAPI Server (Sparse Vector Generation)

New service for generating sparse vectors (BM25-style keyword embeddings) to enable hybrid search combining semantic (dense) and lexical (sparse) retrieval.

```
fit_voyage_splade_service/ (Python — FastAPI)     new service
├── requirements.txt
├── main.py
├── config.py
└── routes/
    └── embeddings.rs    POST /sparse-embeddings — SPLADE model inference
```

### SPLADE Purpose

SPLADE (Sparse Lexical and Dense Embeddings) generates sparse vectors alongside dense embeddings:
- **Dense vectors** (3584 dims from Ollama gte-Qwen2-7B) capture semantic meaning
- **Sparse vectors** (lexical weights for each token) capture exact keyword matches — model numbers (e.g., "MAN 6L28/32H"), part codes, technical terms

Qdrant stores both and combines scores at query time for hybrid retrieval.

### SPLADE Service Structure

```python
# main.py
from fastapi import FastAPI
from transformers import AutoTokenizer, AutoModelForMaskedLM

app = FastAPI()
model_name = "naver/splade-cocondenser-ensembledistil"
tokenizer = AutoTokenizer.from_pretrained(model_name)
model = AutoModelForMaskedLM.from_pretrained(model_name)

@app.post("/sparse-embeddings")
def get_sparse_embeddings(text: str) -> dict:
    inputs = tokenizer(text, return_tensors="pt", padding=True, truncation=True)
    outputs = model(**inputs)
    logits = outputs.logits
    # Apply log(1 + relu(logits)) to get sparse representation
    sparse = {}
    for i, token_id in enumerate(inputs['input_ids'][0]):
        weight = max(0, logits[0][i].max().item())
        if weight > threshold:
            sparse[tokenizer.decode([token_id])] = weight
    return {"sparse_vector": sparse}
```

### Integration with Ingestion Pipeline

```
        ▼ embedder.rs (Rust)
Step 5a: Call both embedders in parallel
  - POST http://localhost:11434/api/embeddings → dense vector (3584 dims)
  - POST http://localhost:8091/sparse-embeddings → sparse vector (token → weight dict)
        │
        ▼ qdrant.rs
Step 6a: Store both in Qdrant PointStruct
  PointStruct {
    id: chunk.id,
    vector: dense_vector,       // 3584 dims (default vector space)
    sparse_vectors: {           // Named sparse vector space
      "bm25": sparse_vector
    },
    payload: { text, vessel_id, doc_name, ... }
  }
```

### Hybrid Search at Query Time

```
POST /ask or POST /search
        │
        ▼ Embed question (dual-path)
  - Dense: POST http://localhost:11434/api/embeddings
  - Sparse: POST http://localhost:8091/sparse-embeddings
        │
        ▼ Qdrant hybrid search
  search_batch([
    {
      vector: dense_question,
      name: "default",           // default vector space
      limit: 20,
      with_filter: compound_OR(...)
    },
    {
      vector: sparse_question,
      name: "bm25",              // sparse vector space
      limit: 20,
      with_filter: compound_OR(...)
    }
  ])
  → Combine scores (0.7 * dense_score + 0.3 * sparse_score)
  → Return top_k (e.g., 5) by combined score
```

**Why this matters:**  
- Query: "What is the MAN 6L28/32H engine issue?"
- Pure semantic search: might return chunks about "engine problems" generically
- Hybrid: weights "MAN 6L28/32H" heavily in sparse score → exact-match chunk ranks first

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
        ▼ embedder.rs (dual-path)
  Parallel calls:
    1. POST http://localhost:11434/api/embeddings  (Ollama — local, no API key)
       model: "gte-Qwen2-7B"
       → Vec<f32>  (3584 dims per chunk)
    
    2. POST http://localhost:8091/sparse-embeddings  (SPLADE FastAPI)
       → Map<String, f32>  (token → weight)
        │
        ▼ qdrant.rs
  collection: "maritime_docs"
  PointStruct {
    id: chunk.id,
    vector: dense_embedding,           // 3584-dim dense vector
    sparse_vectors: {
      "bm25": sparse_embedding         // named sparse vector space
    },
    payload: { text, vessel_id, doc_name, doc_type, chunk_index, doc_scope, scope_id }
  }

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
        ▼ embed the question (embedder.rs — dual-path)
  Parallel calls:
    1. POST http://localhost:11434/api/embeddings, model: "gte-Qwen2-7B"
       → Vec<f32>  (3584 dims, dense)
    
    2. POST http://localhost:8091/sparse-embeddings
       → Map<String, f32>  (token weights, sparse)
        │
        ▼ qdrant.rs — hybrid search
  search_batch on "maritime_docs":
    [
      {
        vector: dense_question,
        name: "default",           // search dense vector space
        limit: 20,
        with_filter: compound OR filter:
          (doc_scope == "vessel"      AND vessel_id    == requested_vessel_id)
          (doc_scope == "vessel_type" AND scope_id     == requested_vessel_type)
          (doc_scope == "design_type" AND scope_id     == requested_design_type)
          (doc_scope == "fleet")
      },
      {
        vector: sparse_question,
        name: "bm25",              // search sparse vector space
        limit: 20,
        with_filter: <same filters>
      }
    ]
  → Vec<ScoredPoint> from both spaces
  
  Combine scores:
    final_score = 0.7 * dense_score + 0.3 * sparse_score
  
  → Top_k: 5 by final_score with payload.text

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

To be put in the qdrant : 
 ID
  - chunk_id — UUID, one per chunk (as you said, to fetch the exact text back after vector search)

  Vector
  - the embedding float array from qwen

  Payload (metadata for filtering + retrieval)
  chunk_text    — the actual text of the chunk (so you don't need a separate DB lookup)
  chunk_index   — position of chunk within the document (0, 1, 2...)
  doc_name      — which document it came from
  doc_type      — incident_report / manual / certificate (filter by doc type)
  doc_scope     — vessel / fleet / vessel_type (filter by scope)
  vessel_id     — which vessel (filter queries to a specific vessel)
  scope_id      — Id number of the scope entity



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
OLLAMA_URL=http://localhost:11434
OLLAMA_EMBED_MODEL=gte-Qwen2-7B
SPLADE_URL=http://localhost:8091
QDRANT_URL=http://localhost:6333
QDRANT_API_KEY=
RUST_LOG=info
PORT=8090
```

**`fit_voyage_splade_service/.env`**
```
MODEL_NAME=naver/splade-cocondenser-ensembledistil
SPLADE_LOG=info
PORT=8091
```

**`fit_voyage_backend_nest/.env` (add)**
```
RAG_SERVICE_URL=http://localhost:8090
```

---

## LLM Observability — LiteLLM Proxy

All LLM calls (currently GPT-4o-mini via `openai.rs`) route through a **LiteLLM proxy** instead of hitting the OpenAI API directly. This gives us centralized logging, latency tracking, cost tracking, and a single place to swap models.

```
openai.rs (Rust)
  └─ POST http://localhost:4000/chat/completions   ← LiteLLM proxy
        │
        ▼ LiteLLM proxy
  - logs: request + response + latency + token counts
  - cost tracking per model
  - forwards to: OpenAI / Ollama / any provider (config-driven)
        │
        ▼ OpenAI API (or local Ollama)
```

### Why LiteLLM

| Concern | What LiteLLM gives us |
|---------|----------------------|
| Latency tracking | Per-request p50/p95/p99 visible in dashboard |
| Cost tracking | Token counts × model pricing per request |
| Model swap | Change `model: gpt-4o-mini` → `model: ollama/llama3` in one config line, zero Rust changes |
| Logging | All prompts + completions persisted for debugging hallucinations or retrieval quality issues |
| Rate limit handling | Retries + fallback models configurable |

### LiteLLM Config

```yaml
# fit_voyage_rag_service/litellm_config.yaml
model_list:
  - model_name: gpt-4o-mini
    litellm_params:
      model: openai/gpt-4o-mini
      api_key: os.environ/OPENAI_API_KEY

general_settings:
  master_key: os.environ/LITELLM_MASTER_KEY

litellm_settings:
  success_callback: ["langfuse"]   # optional: replace with "s3" / "datadog" / none
```

### Quick Start (Local)

```bash
pip install litellm[proxy]
litellm --config fit_voyage_rag_service/litellm_config.yaml --port 4000
# Dashboard at http://localhost:4000/ui
```

### Env Var Change

In `fit_voyage_rag_service/.env`, replace the direct OpenAI URL with the proxy:

```
# Before (direct)
OPENAI_API_URL=https://api.openai.com/v1

# After (via LiteLLM proxy)
OPENAI_API_URL=http://localhost:4000
OPENAI_API_KEY=<your key>             # still needed; proxy forwards it
LITELLM_MASTER_KEY=<proxy admin key>  # for the dashboard
```

`openai.rs` needs no other changes — it already calls a configurable `OPENAI_API_URL`.

---

## Build Sequence

| Step | Task | Verify |
|------|------|--------|
| 1 | Rust scaffold — Axum server on :8090 | `GET /health` → 200 |
| 2 | `extractor.rs` — upload PDF, get raw text | POST /documents returns extracted text in debug mode |
| 3 | `chunker.rs` — naive word-count chunks (500 words, 50 overlap) → Vec<Chunk> | unit test: chunk count correct, overlap present |
| 3a | **Improve: semantic chunking** — split text into sentences first; embed each sentence; walk sequentially and compare each new sentence against the **centroid** (average embedding) of the current chunk so far. If cosine similarity < 0.75 OR chunk exceeds 400 words → close chunk, start new one. Centroid is more stable than comparing adjacent sentences directly because it captures the overall topic of the accumulated group, not just the last sentence. | chunks respect topic boundaries; no mid-sentence cuts; related sentences stay together |
| 4 | Qdrant via Docker — create `maritime_docs` collection with named sparse vector space "bm25" | Qdrant dashboard at :6333 shows collection with both dense and sparse vector support |
| 5 | `embedder.rs` — call Ollama gte-Qwen2-7B embeddings (dense vectors) | assert `embedding.len() == 3584` |


| 5a | **SPLADE FastAPI server on :8091** — generate sparse vectors (lexical embeddings) for each chunk in parallel with dense embeddings. Model: naver/splade-cocondenser-ensembledistil. Sparse vectors weight individual tokens by relevance. | `POST /sparse-embeddings "MAN 6L28/32H engine failure"` returns `{"MAN": 0.92, "6L28": 0.88, "engine": 0.75, "failure": 0.68, ...}` |
| 6 | `qdrant.rs` — upsert with dual vectors: dense (default space) + sparse (named "bm25" space) | Qdrant point has both `.vector` and `.sparse_vectors["bm25"]` populated |


6a1.1 | We will be creating two collections one with sparse vectors and 1 with dense vectors . Sparse vectors will have the parent id in the payload for retrieval . 

  On query , fetch top k in sparse and (re fetch their parent) . Fetch top k in dense collection .
  Merge them by id's first . Then the final list by reciprocal rank fusion since we would be having a rank from either fetches . 

  What if two sparse ones belong to the same parent id ?  we add their contrbution by 1/(rank1 + k) 
  1/(rank2 + k). 


  






| 6a | **Hybrid search at query time** — call both embedders in parallel (dense + sparse); search Qdrant with both vector spaces; combine scores (0.7 * dense_score + 0.3 * sparse_score) for final ranking. Catches both semantic meaning and exact keyword matches. | searching "MAN 6L28/32H" weights lexical match heavily → exact-match chunk ranks first even if semantic similarity is moderate |
| 7 | `openai.rs` — full RAG prompt + GPT call | `POST /ask` returns grounded answer + source_chunks (doc_name, chunk_index per chunk) |
| 7a | **Improve: re-ranking** — after fetching top 20 from Qdrant, run a cross-encoder model to re-score and select the best 5 to send to LLM. Cross-encoders compare query+chunk together (not independently) so relevance scoring is far more accurate than cosine similarity alone. | top 5 sent to LLM are measurably more relevant than the raw hybrid-scored top 5 |



| 7b | **Improve: HyDE (Hypothetical Document Embedding)** — before embedding the question, ask the LLM to write a short hypothetical answer; embed that instead (both dense and sparse). Answers live closer to answers in vector space than questions do, so retrieval improves. | retrieved chunks match answer-shaped content better than raw question embedding |


| 7c | **Improve: query expansion** — LLM rewrites the user question into 3-4 specific variants; search with all of them (hybrid for each) and union the results before re-ranking. Handles vague or short queries. | "what went wrong with the engine?" retrieves chunks that a single embedding would miss |
| 8 | NestJS proxy wired into AiAnalyticsModule | `POST /api/ai/rag/ask` passes through correctly |
| 9 | Frontend upload + chat UI | Upload PDF → ask question → see answer with citations |

---

## MVP Milestone (Step 7)

Upload a real maritime incident report PDF for a vessel.  
Call `POST /ask { "question": "What happened?", "vessel_id": ... }`.  
Receive a grounded answer with `source_chunks` naming the document and chunk index.

That is the complete RAG loop.

---

## SPLADE Service Deployment

### Python Requirements

```txt
# fit_voyage_splade_service/requirements.txt
fastapi==0.104.0
uvicorn[standard]==0.24.0
torch==2.1.0
transformers==4.34.0
numpy==1.24.0
```

### Quick Start (Local)

```bash
# Terminal 1: Start SPLADE service
cd fit_voyage_splade_service
python -m venv venv
source venv/bin/activate  # or venv\Scripts\activate on Windows
pip install -r requirements.txt
python main.py
# Server runs on http://localhost:8091

# Test endpoint
curl -X POST "http://localhost:8091/sparse-embeddings" \
  -H "Content-Type: application/json" \
  -d '{"text": "MAN 6L28/32H engine bearing wear"}'

# Expected response
{
  "sparse_vector": {
    "MAN": 0.92,
    "6L28": 0.88,
    "engine": 0.75,
    "bearing": 0.82,
    "wear": 0.68
  }
}
```

### Docker Deployment (Optional for later phases)

```dockerfile
# fit_voyage_splade_service/Dockerfile
FROM python:3.11-slim
WORKDIR /app
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt
COPY . .
CMD ["uvicorn", "main:app", "--host", "0.0.0.0", "--port", "8091"]
```

---

## Deferred (Phase 2+)

| Feature | Why deferred |
|---------|-------------|
| Local LLM (Ollama) | CUDA/VRAM rabbit hole; OpenAI is $0.001/call |
| Re-ranking (cross-encoders) | Phase 2 retrieval quality optimization after hybrid search working |
| Streaming responses | Nice UX, not required for correctness |
| LangChain/LlamaIndex Rust crates | Learn primitives first, frameworks later |
| pgvector instead of Qdrant | Qdrant has better Rust client + vessel_id filter support + sparse vector support |










## Advanced Techniques Reference

Grouped by where in the pipeline they apply. The "a/b/c" sub-steps in the Build Sequence above map to these.

### Chunking

| Technique | What it does | Build step |
|-----------|-------------|------------|
| Sentence-aware chunking | Split on sentence boundaries before applying size limits — no mid-sentence cuts | 3a |
| Semantic chunking (centroid-based) | Embed each sentence; maintain a centroid (average of all embeddings in the current chunk); compare new sentence against centroid not the previous sentence — centroid is stable across topic-consistent sentences, drops sharply when meaning shifts. Split on drop below threshold (0.75) OR max word count (400). Per-sentence embeddings are temporary and discarded after chunking; the final chunk is re-embedded as one unit for Qdrant. | 3a |

### Retrieval

| Technique | What it does | Build step |
|-----------|-------------|------------|
| Higher top_k | Fetch top 20, send top 5 to LLM — more chances of catching scattered chunks | 6 |
| Hybrid search | Dense vectors (semantic) + sparse vectors (BM25 keyword) combined — catches both meaning and exact terms like model numbers | 6a |
| Structured filters | Filter by vessel_id, doc_type, doc_scope — narrows search space before vector scoring | 8 (after MVP) |
| Knowledge graphs | Post-ingestion step: after all chunks are stored, compare each chunk against all others; if two chunks score high cosine similarity but are far apart in the document, store their IDs as linked in each other's Qdrant payload (`related_chunk_ids: [uuid, uuid]`). At retrieval time, when chunk A is fetched, chunk C is pulled automatically even if the query didn't directly match it. Solves the topic-interleaving problem (related paragraphs separated by unrelated content). | Phase 2 |

### Re-ranking

| Technique | What it does | Build step |
|-----------|-------------|------------|
| Cross-encoder re-ranking | After fetching top 20, run a cross-encoder to jointly score query+chunk pairs and pick the best 5 — far more accurate than cosine similarity alone | 7a |

### Query quality

| Technique | What it does | Build step |
|-----------|-------------|------------|
| HyDE (Hypothetical Document Embedding) | LLM generates a hypothetical answer first; embed that instead of the raw question — answers live closer to answers in vector space | 7b |
| Query expansion | LLM rewrites the question into 3-4 variants; search all of them and union results before re-ranking | 7c |
| Conversation history | Include prior messages so "what happened with it?" resolves correctly if the user already said "it = the coolant pipe" | Phase 2 |
