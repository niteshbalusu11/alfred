# Assistant Encrypted Ingress Protocol v1

Last updated: 2026-02-16
Issue scope: #147

## Goal

Assistant message content enters backend as ciphertext and is decrypted only in enclave runtime.

## API Flow

1. Client sends `POST /v1/assistant/attested-key` with challenge fields:
   - `challenge_nonce`
   - `issued_at`
   - `expires_at`
   - `request_id`
2. API forwards challenge to enclave RPC endpoint `/v1/rpc/assistant/attested-key`.
3. Enclave responds with:
   - `key_id`
   - `algorithm` (`x25519-chacha20poly1305`)
   - `public_key` (X25519)
   - `key_expires_at`
   - attestation evidence (`runtime`, `measurement`, challenge echo fields, `evidence_issued_at`, `signature`)
4. Client verifies attestation and key binding.
5. Client encrypts assistant plaintext request and sends `POST /v1/assistant/query` with envelope-only payload.
6. API validates envelope shape/metadata, forwards to enclave RPC `/v1/rpc/assistant/query`, and returns encrypted response envelope.

## Attestation + Key Binding Verification Rules (Client)

The client must fail closed unless all checks pass:

1. `algorithm` equals `x25519-chacha20poly1305`.
2. Challenge echo fields match exactly:
   - `challenge_nonce`
   - `request_id`
3. Challenge window is valid:
   - `expires_at > issued_at`
   - current time `<= expires_at`
4. Runtime identity checks:
   - `runtime` equals expected runtime
   - `measurement` is in allowed enclave measurement allowlist
5. Evidence freshness and challenge binding:
   - `issued_at <= evidence_issued_at <= expires_at`
   - `abs(now - evidence_issued_at) <= max_attestation_age_seconds`
6. Key freshness:
   - `key_expires_at >= now`
7. Signature verification:
   - verify Ed25519 signature over payload:
     `runtime|measurement|challenge_nonce|issued_at|expires_at|request_id|evidence_issued_at|key_id|algorithm|public_key|key_expires_at`

## Envelope v1

Request envelope (`AssistantEncryptedRequestEnvelope`):

- `version` = `v1`
- `algorithm` = `x25519-chacha20poly1305`
- `key_id`
- `request_id`
- `client_ephemeral_public_key` (base64 X25519 public key)
- `nonce` (base64 12-byte nonce)
- `ciphertext` (base64 ChaCha20-Poly1305 combined ciphertext)

Response envelope (`AssistantEncryptedResponseEnvelope`) mirrors version/algo/key/request metadata and contains encrypted payload fields (`nonce`, `ciphertext`).

## Host Privacy Boundary (Phase 1)

Host API is limited to:

1. Envelope metadata validation.
2. Opaque ciphertext forwarding to enclave runtime.
3. Opaque encrypted session-state persistence for continuity primitives.

Host API does not parse assistant plaintext request content.
