import CryptoKit
import Foundation
import Security

public struct AssistantAttestationVerificationConfig: Sendable {
    public let expectedRuntime: String
    public let allowedMeasurements: Set<String>
    public let attestationPublicKeyBase64: String
    public let maxAttestationAgeSeconds: Int64
    public let challengeWindowSeconds: Int

    public init(
        expectedRuntime: String,
        allowedMeasurements: Set<String>,
        attestationPublicKeyBase64: String,
        maxAttestationAgeSeconds: Int64 = 300,
        challengeWindowSeconds: Int = 30
    ) {
        self.expectedRuntime = expectedRuntime
        self.allowedMeasurements = allowedMeasurements
        self.attestationPublicKeyBase64 = attestationPublicKeyBase64
        self.maxAttestationAgeSeconds = maxAttestationAgeSeconds
        self.challengeWindowSeconds = challengeWindowSeconds
    }
}

struct EncryptedAssistantRequestPayload {
    let envelope: AssistantEncryptedRequestEnvelope
    let clientEphemeralPrivateKey: Data
}

enum AssistantEnvelopeCrypto {
    static let versionV1 = "v1"
    static let algorithmX25519ChaCha20Poly1305 = "x25519-chacha20poly1305"

    static func verifyAttestedKeyResponse(
        _ response: AssistantAttestedKeyResponse,
        expectedChallengeNonce: String,
        expectedRequestID: String,
        config: AssistantAttestationVerificationConfig
    ) throws {
        guard response.algorithm == algorithmX25519ChaCha20Poly1305 else {
            throw AlfredAPIClientError.assistantAttestationFailed(reason: "unsupported key algorithm")
        }
        guard response.attestation.challengeNonce == expectedChallengeNonce else {
            throw AlfredAPIClientError.assistantAttestationFailed(reason: "challenge nonce mismatch")
        }
        guard response.attestation.requestId == expectedRequestID else {
            throw AlfredAPIClientError.assistantAttestationFailed(reason: "request_id mismatch")
        }
        guard response.attestation.expiresAt > response.attestation.issuedAt else {
            throw AlfredAPIClientError.assistantAttestationFailed(reason: "invalid attestation challenge window")
        }
        guard response.attestation.runtime.caseInsensitiveCompare(config.expectedRuntime) == .orderedSame else {
            throw AlfredAPIClientError.assistantAttestationFailed(reason: "unexpected enclave runtime")
        }
        guard config.allowedMeasurements.contains(response.attestation.measurement) else {
            throw AlfredAPIClientError.assistantAttestationFailed(reason: "measurement is not allowed")
        }

        let now = Int64(Date().timeIntervalSince1970)
        guard now <= response.attestation.expiresAt else {
            throw AlfredAPIClientError.assistantAttestationFailed(reason: "challenge expired")
        }
        guard response.attestation.evidenceIssuedAt >= response.attestation.issuedAt,
              response.attestation.evidenceIssuedAt <= response.attestation.expiresAt else {
            throw AlfredAPIClientError.assistantAttestationFailed(reason: "evidence is not challenge-bound")
        }
        guard abs(now - response.attestation.evidenceIssuedAt) <= config.maxAttestationAgeSeconds else {
            throw AlfredAPIClientError.assistantAttestationFailed(reason: "attestation evidence is stale")
        }
        guard response.keyExpiresAt >= now else {
            throw AlfredAPIClientError.assistantAttestationFailed(reason: "attested key has expired")
        }

        guard let encodedPublicKey = Data(base64Encoded: config.attestationPublicKeyBase64),
              encodedPublicKey.count == 32 else {
            throw AlfredAPIClientError.assistantAttestationFailed(reason: "attestation verification key is invalid")
        }
        guard let signatureB64 = response.attestation.signature,
              let signature = Data(base64Encoded: signatureB64) else {
            throw AlfredAPIClientError.assistantAttestationFailed(reason: "attestation signature missing")
        }

        let payload = assistantKeyAttestationSigningPayload(response)
        let publicKey: Curve25519.Signing.PublicKey
        do {
            publicKey = try Curve25519.Signing.PublicKey(rawRepresentation: encodedPublicKey)
        } catch {
            throw AlfredAPIClientError.assistantAttestationFailed(reason: "attestation verification key parse failed")
        }

        guard publicKey.isValidSignature(signature, for: Data(payload.utf8)) else {
            throw AlfredAPIClientError.assistantAttestationFailed(reason: "attestation signature is invalid")
        }
    }

    static func encryptRequest(
        plaintextRequest: AssistantPlaintextQueryRequest,
        requestID: String,
        attestedKey: AssistantAttestedKeyResponse
    ) throws -> EncryptedAssistantRequestPayload {
        guard attestedKey.algorithm == algorithmX25519ChaCha20Poly1305 else {
            throw AlfredAPIClientError.assistantEncryptionFailed(reason: "unsupported key algorithm")
        }
        guard let enclavePublicKeyRaw = Data(base64Encoded: attestedKey.publicKey), enclavePublicKeyRaw.count == 32 else {
            throw AlfredAPIClientError.assistantEncryptionFailed(reason: "invalid enclave public key")
        }

        let clientEphemeralPrivateKey = Curve25519.KeyAgreement.PrivateKey()
        let enclavePublicKey: Curve25519.KeyAgreement.PublicKey
        do {
            enclavePublicKey = try Curve25519.KeyAgreement.PublicKey(rawRepresentation: enclavePublicKeyRaw)
        } catch {
            throw AlfredAPIClientError.assistantEncryptionFailed(reason: "failed to parse enclave key")
        }

        let sharedSecret: SharedSecret
        do {
            sharedSecret = try clientEphemeralPrivateKey.sharedSecretFromKeyAgreement(with: enclavePublicKey)
        } catch {
            throw AlfredAPIClientError.assistantEncryptionFailed(reason: "key agreement failed")
        }

        let symmetricKey = deriveDirectionalSymmetricKey(
            sharedSecret: sharedSecret,
            requestID: requestID,
            direction: "request"
        )
        let requestNonce = try randomNonceData()
        let nonce: ChaChaPoly.Nonce
        do {
            nonce = try ChaChaPoly.Nonce(data: requestNonce)
        } catch {
            throw AlfredAPIClientError.assistantEncryptionFailed(reason: "failed to create nonce")
        }

        let plaintext: Data
        do {
            plaintext = try JSONEncoder().encode(plaintextRequest)
        } catch {
            throw AlfredAPIClientError.assistantEncryptionFailed(reason: "failed to encode plaintext request")
        }

        let sealedBox: ChaChaPoly.SealedBox
        do {
            sealedBox = try ChaChaPoly.seal(
                plaintext,
                using: symmetricKey,
                nonce: nonce,
                authenticating: Data(requestID.utf8)
            )
        } catch {
            throw AlfredAPIClientError.assistantEncryptionFailed(reason: "request encryption failed")
        }

        return EncryptedAssistantRequestPayload(
            envelope: AssistantEncryptedRequestEnvelope(
                version: versionV1,
                algorithm: algorithmX25519ChaCha20Poly1305,
                keyId: attestedKey.keyId,
                requestId: requestID,
                clientEphemeralPublicKey: clientEphemeralPrivateKey.publicKey.rawRepresentation.base64EncodedString(),
                nonce: requestNonce.base64EncodedString(),
                ciphertext: sealedBox.combined.base64EncodedString()
            ),
            clientEphemeralPrivateKey: clientEphemeralPrivateKey.rawRepresentation
        )
    }

    static func decryptResponse(
        envelope: AssistantEncryptedResponseEnvelope,
        requestID: String,
        clientEphemeralPrivateKey: Data,
        attestedKey: AssistantAttestedKeyResponse
    ) throws -> AssistantPlaintextQueryResponse {
        guard envelope.version == versionV1 else {
            throw AlfredAPIClientError.assistantDecryptionFailed(reason: "unsupported envelope version")
        }
        guard envelope.algorithm == algorithmX25519ChaCha20Poly1305 else {
            throw AlfredAPIClientError.assistantDecryptionFailed(reason: "unsupported envelope algorithm")
        }
        guard envelope.keyId == attestedKey.keyId else {
            throw AlfredAPIClientError.assistantDecryptionFailed(reason: "key_id mismatch")
        }
        guard envelope.requestId == requestID else {
            throw AlfredAPIClientError.assistantDecryptionFailed(reason: "request_id mismatch")
        }

        guard let nonceData = Data(base64Encoded: envelope.nonce), nonceData.count == 12 else {
            throw AlfredAPIClientError.assistantDecryptionFailed(reason: "response nonce is invalid")
        }
        guard let ciphertext = Data(base64Encoded: envelope.ciphertext) else {
            throw AlfredAPIClientError.assistantDecryptionFailed(reason: "response ciphertext is invalid")
        }
        guard let enclavePublicKeyRaw = Data(base64Encoded: attestedKey.publicKey), enclavePublicKeyRaw.count == 32 else {
            throw AlfredAPIClientError.assistantDecryptionFailed(reason: "enclave public key is invalid")
        }

        let clientPrivateKey: Curve25519.KeyAgreement.PrivateKey // gitleaks:allow
        do {
            clientPrivateKey = try Curve25519.KeyAgreement.PrivateKey(rawRepresentation: clientEphemeralPrivateKey)
        } catch {
            throw AlfredAPIClientError.assistantDecryptionFailed(reason: "client ephemeral key is invalid")
        }

        let enclavePublicKey: Curve25519.KeyAgreement.PublicKey
        do {
            enclavePublicKey = try Curve25519.KeyAgreement.PublicKey(rawRepresentation: enclavePublicKeyRaw)
        } catch {
            throw AlfredAPIClientError.assistantDecryptionFailed(reason: "enclave key parse failed")
        }

        let sharedSecret: SharedSecret
        do {
            sharedSecret = try clientPrivateKey.sharedSecretFromKeyAgreement(with: enclavePublicKey)
        } catch {
            throw AlfredAPIClientError.assistantDecryptionFailed(reason: "key agreement failed")
        }

        let symmetricKey = deriveDirectionalSymmetricKey(
            sharedSecret: sharedSecret,
            requestID: requestID,
            direction: "response"
        )

        let nonce: ChaChaPoly.Nonce
        do {
            nonce = try ChaChaPoly.Nonce(data: nonceData)
        } catch {
            throw AlfredAPIClientError.assistantDecryptionFailed(reason: "nonce parse failed")
        }

        let sealedBox: ChaChaPoly.SealedBox
        do {
            sealedBox = try ChaChaPoly.SealedBox(combined: ciphertext)
        } catch {
            throw AlfredAPIClientError.assistantDecryptionFailed(reason: "sealed box parse failed")
        }

        let plaintext: Data
        do {
            plaintext = try ChaChaPoly.open(
                sealedBox,
                using: symmetricKey,
                authenticating: Data(requestID.utf8)
            )
        } catch {
            throw AlfredAPIClientError.assistantDecryptionFailed(reason: "response decryption failed")
        }

        do {
            return try JSONDecoder().decode(AssistantPlaintextQueryResponse.self, from: plaintext)
        } catch {
            throw AlfredAPIClientError.assistantDecryptionFailed(reason: "response payload decode failed")
        }
    }

    private static func deriveDirectionalSymmetricKey(
        sharedSecret: SharedSecret,
        requestID: String,
        direction: String
    ) -> SymmetricKey {
        let sharedSecretData = sharedSecret.withUnsafeBytes { Data($0) }
        var digestInput = Data()
        digestInput.append(sharedSecretData)
        digestInput.append(Data("|".utf8))
        digestInput.append(Data(requestID.utf8))
        digestInput.append(Data("|".utf8))
        digestInput.append(Data(direction.utf8))
        let digest = SHA256.hash(data: digestInput)
        return SymmetricKey(data: Data(digest))
    }

    private static func assistantKeyAttestationSigningPayload(_ response: AssistantAttestedKeyResponse) -> String {
        [
            response.attestation.runtime,
            response.attestation.measurement,
            response.attestation.challengeNonce,
            String(response.attestation.issuedAt),
            String(response.attestation.expiresAt),
            response.attestation.requestId,
            String(response.attestation.evidenceIssuedAt),
            response.keyId,
            response.algorithm,
            response.publicKey,
            String(response.keyExpiresAt)
        ]
        .joined(separator: "|")
    }

    private static func randomNonceData() throws -> Data {
        var bytes = [UInt8](repeating: 0, count: 12)
        let status = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        guard status == errSecSuccess else {
            throw AlfredAPIClientError.assistantEncryptionFailed(reason: "random nonce generation failed")
        }
        return Data(bytes)
    }
}
