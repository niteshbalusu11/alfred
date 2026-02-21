import AlfredAPIClient
import CryptoKit
import XCTest

final class AutomationNotificationCryptoTests: XCTestCase {
    func testRegistrationMaterialAndDecryptionMaterialRemainAligned() throws {
        let registration = try AutomationNotificationCrypto.registrationMaterial()
        let decryption = try XCTUnwrap(AutomationNotificationCrypto.decryptionMaterial())

        XCTAssertEqual(registration.deviceID, decryption.deviceID)
        XCTAssertEqual(
            registration.algorithm,
            AutomationNotificationCrypto.algorithmX25519ChaCha20Poly1305
        )
        XCTAssertFalse(registration.publicKey.isEmpty)
    }

    func testResolveVisibleContentDecryptsValidPayload() async throws {
        let registration = try AutomationNotificationCrypto.registrationMaterial()
        let plaintext = AutomationNotificationContent(title: "Build status", body: "Everything is healthy.")
        let envelope = try makeEnvelope(
            plaintext: plaintext,
            deviceID: registration.deviceID,
            recipientPublicKeyBase64: registration.publicKey
        )

        let userInfo: [AnyHashable: Any] = [
            "alfred_automation": [
                "version": AutomationNotificationCrypto.versionV1,
                "envelope": [
                    "version": envelope.version,
                    "algorithm": envelope.algorithm,
                    "key_id": envelope.keyID,
                    "request_id": envelope.requestID,
                    "sender_public_key": envelope.senderPublicKey,
                    "nonce": envelope.nonce,
                    "ciphertext": envelope.ciphertext,
                ],
            ],
        ]

        let resolved = await AutomationNotificationCrypto.resolveVisibleContent(from: userInfo)
        XCTAssertEqual(resolved, plaintext)
    }

    func testResolveVisibleContentFallsBackOnInvalidPayload() async {
        let userInfo: [AnyHashable: Any] = ["unexpected": "value"]
        let resolved = await AutomationNotificationCrypto.resolveVisibleContent(from: userInfo)

        XCTAssertEqual(resolved, .fallback)
    }

    func testResolveVisibleContentFallsBackOnTimeout() async throws {
        let _ = try AutomationNotificationCrypto.registrationMaterial()
        let userInfo: [AnyHashable: Any] = [
            "alfred_automation": [
                "version": AutomationNotificationCrypto.versionV1,
                "envelope": [
                    "version": AutomationNotificationCrypto.versionV1,
                    "algorithm": AutomationNotificationCrypto.algorithmX25519ChaCha20Poly1305,
                    "key_id": "sender-key",
                    "request_id": "req-timeout",
                    "sender_public_key": Data(repeating: 7, count: 32).base64EncodedString(),
                    "nonce": Data(repeating: 9, count: 12).base64EncodedString(),
                    "ciphertext": Data(repeating: 5, count: 32).base64EncodedString(),
                ],
            ],
        ]

        let resolved = await AutomationNotificationCrypto.resolveVisibleContent(
            from: userInfo,
            timeoutNanoseconds: 10_000_000,
            decryptor: { _, _ in
                try await Task.sleep(nanoseconds: 300_000_000)
                return AutomationNotificationContent(title: "Late", body: "Result")
            }
        )

        XCTAssertEqual(resolved, .fallback)
    }

    func testNotificationPreviewTruncatesLongContent() {
        let content = AutomationNotificationContent(
            title: String(repeating: "T", count: 80),
            body: String(repeating: "B", count: 220)
        )

        let preview = AutomationNotificationPreview.makeVisiblePreview(from: content)

        XCTAssertEqual(preview.title.count, 67)
        XCTAssertEqual(preview.body.count, 183)
        XCTAssertTrue(preview.title.hasSuffix("..."))
        XCTAssertTrue(preview.body.hasSuffix("..."))
    }

    private func makeEnvelope(
        plaintext: AutomationNotificationContent,
        deviceID: String,
        recipientPublicKeyBase64: String
    ) throws -> AutomationEncryptedNotificationEnvelope {
        let senderPrivateKey = Curve25519.KeyAgreement.PrivateKey()
        let requestID = UUID().uuidString.lowercased()

        guard let recipientPublicKeyData = Data(base64Encoded: recipientPublicKeyBase64), recipientPublicKeyData.count == 32 else {
            XCTFail("recipient public key must decode to 32 bytes")
            throw TestError.invalidFixture
        }
        let recipientPublicKey = try Curve25519.KeyAgreement.PublicKey(rawRepresentation: recipientPublicKeyData)

        let sharedSecret = try senderPrivateKey.sharedSecretFromKeyAgreement(with: recipientPublicKey)
        let symmetricKey = deriveSymmetricKey(
            sharedSecret: sharedSecret,
            requestID: requestID,
            deviceID: deviceID
        )

        let nonceData = randomNonceData()
        let nonce = try ChaChaPoly.Nonce(data: nonceData)
        let aad = Data("\(requestID)|\(deviceID)".utf8)
        let plaintextData = try JSONEncoder().encode(plaintext)
        let sealedBox = try ChaChaPoly.seal(
            plaintextData,
            using: symmetricKey,
            nonce: nonce,
            authenticating: aad
        )

        var ciphertextWithTag = Data()
        ciphertextWithTag.append(sealedBox.ciphertext)
        ciphertextWithTag.append(sealedBox.tag)

        return AutomationEncryptedNotificationEnvelope(
            version: AutomationNotificationCrypto.versionV1,
            algorithm: AutomationNotificationCrypto.algorithmX25519ChaCha20Poly1305,
            keyID: "sender-key",
            requestID: requestID,
            senderPublicKey: senderPrivateKey.publicKey.rawRepresentation.base64EncodedString(),
            nonce: nonceData.base64EncodedString(),
            ciphertext: ciphertextWithTag.base64EncodedString()
        )
    }

    private func deriveSymmetricKey(
        sharedSecret: SharedSecret,
        requestID: String,
        deviceID: String
    ) -> SymmetricKey {
        let secretData = sharedSecret.withUnsafeBytes { Data($0) }
        var digestInput = Data()
        digestInput.append(secretData)
        digestInput.append(Data("|".utf8))
        digestInput.append(Data(requestID.utf8))
        digestInput.append(Data("|".utf8))
        digestInput.append(Data(deviceID.utf8))
        digestInput.append(Data("|notification".utf8))
        let digest = SHA256.hash(data: digestInput)
        return SymmetricKey(data: Data(digest))
    }

    private func randomNonceData() -> Data {
        var bytes = [UInt8](repeating: 0, count: 12)
        for index in bytes.indices {
            bytes[index] = UInt8.random(in: 0...255)
        }
        return Data(bytes)
    }

    private enum TestError: Error {
        case invalidFixture
    }
}
