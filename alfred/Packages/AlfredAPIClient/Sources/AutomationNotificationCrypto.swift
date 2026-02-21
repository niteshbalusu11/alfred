import CryptoKit
import Foundation
import Security

public struct AutomationNotificationContent: Codable, Equatable, Sendable {
    public let title: String
    public let body: String

    public init(title: String, body: String) {
        self.title = title
        self.body = body
    }

    public static let fallback = AutomationNotificationContent(
        title: "Automation update",
        body: "Open Alfred to view your latest automation result."
    )
}

public struct AutomationNotificationRegistrationMaterial: Equatable, Sendable {
    public let deviceID: String
    public let algorithm: String
    public let publicKey: String

    public init(deviceID: String, algorithm: String, publicKey: String) {
        self.deviceID = deviceID
        self.algorithm = algorithm
        self.publicKey = publicKey
    }
}

public struct AutomationNotificationDecryptionMaterial: Equatable, Sendable {
    public let deviceID: String
    public let privateKeyRawRepresentation: Data

    public init(deviceID: String, privateKeyRawRepresentation: Data) {
        self.deviceID = deviceID
        self.privateKeyRawRepresentation = privateKeyRawRepresentation
    }
}

public struct AutomationEncryptedNotificationEnvelope: Codable, Equatable, Sendable {
    public let version: String
    public let algorithm: String
    public let keyID: String
    public let requestID: String
    public let senderPublicKey: String
    public let nonce: String
    public let ciphertext: String

    enum CodingKeys: String, CodingKey {
        case version
        case algorithm
        case keyID = "key_id"
        case requestID = "request_id"
        case senderPublicKey = "sender_public_key"
        case nonce
        case ciphertext
    }

    public init(
        version: String,
        algorithm: String,
        keyID: String,
        requestID: String,
        senderPublicKey: String,
        nonce: String,
        ciphertext: String
    ) {
        self.version = version
        self.algorithm = algorithm
        self.keyID = keyID
        self.requestID = requestID
        self.senderPublicKey = senderPublicKey
        self.nonce = nonce
        self.ciphertext = ciphertext
    }
}

public enum AutomationNotificationCryptoError: Error {
    case keychain(OSStatus)
    case invalidKeyMaterial
    case payloadInvalid
    case envelopeMissing
    case unsupportedVersion
    case unsupportedAlgorithm
    case invalidSenderPublicKey
    case invalidNonce
    case invalidCiphertext
    case keyAgreementFailed
    case decryptionFailed
    case plaintextInvalid
}

public enum AutomationNotificationCrypto {
    public static let versionV1 = "v1"
    public static let algorithmX25519ChaCha20Poly1305 = "x25519-chacha20poly1305"

    private static let keychainService = "com.prodata.alfred.automation-notification"
    private static let keychainAccount = "device-key-material-v1"

    private struct StoredKeyMaterial: Codable {
        let deviceID: String
        let privateKeyBase64: String
    }

    private struct PayloadRoot: Codable {
        let alfredAutomation: PayloadContainer

        enum CodingKeys: String, CodingKey {
            case alfredAutomation = "alfred_automation"
        }
    }

    private struct PayloadContainer: Codable {
        let version: String
        let envelope: AutomationEncryptedNotificationEnvelope
    }

    private struct NotificationPlaintext: Codable {
        let title: String
        let body: String
    }

    public static func registrationMaterial() throws -> AutomationNotificationRegistrationMaterial {
        let stored = try loadOrCreateStoredKeyMaterial()
        let privateKeyData = try decodePrivateKeyData(base64Encoded: stored.privateKeyBase64)

        let privateKey: Curve25519.KeyAgreement.PrivateKey
        do {
            privateKey = try Curve25519.KeyAgreement.PrivateKey(rawRepresentation: privateKeyData)
        } catch {
            throw AutomationNotificationCryptoError.invalidKeyMaterial
        }

        return AutomationNotificationRegistrationMaterial(
            deviceID: stored.deviceID,
            algorithm: algorithmX25519ChaCha20Poly1305,
            publicKey: privateKey.publicKey.rawRepresentation.base64EncodedString()
        )
    }

    public static func decryptionMaterial() throws -> AutomationNotificationDecryptionMaterial? {
        guard let stored = try loadStoredKeyMaterial() else {
            return nil
        }
        let privateKeyData = try decodePrivateKeyData(base64Encoded: stored.privateKeyBase64)

        return AutomationNotificationDecryptionMaterial(
            deviceID: stored.deviceID,
            privateKeyRawRepresentation: privateKeyData
        )
    }

    public static func encryptedEnvelope(from userInfo: [AnyHashable: Any]) throws -> AutomationEncryptedNotificationEnvelope {
        let jsonObject = normalizeUserInfo(userInfo)
        guard JSONSerialization.isValidJSONObject(jsonObject) else {
            throw AutomationNotificationCryptoError.payloadInvalid
        }

        let payloadData = try JSONSerialization.data(withJSONObject: jsonObject, options: [])
        let payload = try JSONDecoder().decode(PayloadRoot.self, from: payloadData)

        guard payload.alfredAutomation.version == versionV1,
              payload.alfredAutomation.envelope.version == versionV1 else {
            throw AutomationNotificationCryptoError.unsupportedVersion
        }

        return payload.alfredAutomation.envelope
    }

    public static func requestID(from userInfo: [AnyHashable: Any]) -> String? {
        guard let value = try? encryptedEnvelope(from: userInfo).requestID else {
            return nil
        }
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }

    public static func decrypt(
        envelope: AutomationEncryptedNotificationEnvelope,
        material: AutomationNotificationDecryptionMaterial
    ) throws -> AutomationNotificationContent {
        guard envelope.version == versionV1 else {
            throw AutomationNotificationCryptoError.unsupportedVersion
        }
        guard envelope.algorithm == algorithmX25519ChaCha20Poly1305 else {
            throw AutomationNotificationCryptoError.unsupportedAlgorithm
        }

        guard let nonceData = Data(base64Encoded: envelope.nonce), nonceData.count == 12 else {
            throw AutomationNotificationCryptoError.invalidNonce
        }
        guard let ciphertextWithTag = Data(base64Encoded: envelope.ciphertext), ciphertextWithTag.count >= 16 else {
            throw AutomationNotificationCryptoError.invalidCiphertext
        }
        guard let senderPublicKeyData = Data(base64Encoded: envelope.senderPublicKey), senderPublicKeyData.count == 32 else {
            throw AutomationNotificationCryptoError.invalidSenderPublicKey
        }

        let recipientPrivateKey: Curve25519.KeyAgreement.PrivateKey
        let senderPublicKey: Curve25519.KeyAgreement.PublicKey
        do {
            recipientPrivateKey = try Curve25519.KeyAgreement.PrivateKey(rawRepresentation: material.privateKeyRawRepresentation)
            senderPublicKey = try Curve25519.KeyAgreement.PublicKey(rawRepresentation: senderPublicKeyData)
        } catch {
            throw AutomationNotificationCryptoError.invalidKeyMaterial
        }

        let sharedSecret: SharedSecret
        do {
            sharedSecret = try recipientPrivateKey.sharedSecretFromKeyAgreement(with: senderPublicKey)
        } catch {
            throw AutomationNotificationCryptoError.keyAgreementFailed
        }

        let derivedKey = deriveNotificationSymmetricKey(
            sharedSecret: sharedSecret,
            requestID: envelope.requestID,
            deviceID: material.deviceID
        )
        let aad = Data("\(envelope.requestID)|\(material.deviceID)".utf8)

        let nonce: ChaChaPoly.Nonce
        do {
            nonce = try ChaChaPoly.Nonce(data: nonceData)
        } catch {
            throw AutomationNotificationCryptoError.invalidNonce
        }

        let encryptedBytes = ciphertextWithTag.prefix(ciphertextWithTag.count - 16)
        let tag = ciphertextWithTag.suffix(16)

        let sealedBox: ChaChaPoly.SealedBox
        do {
            sealedBox = try ChaChaPoly.SealedBox(
                nonce: nonce,
                ciphertext: Data(encryptedBytes),
                tag: Data(tag)
            )
        } catch {
            throw AutomationNotificationCryptoError.invalidCiphertext
        }

        let plaintext: Data
        do {
            plaintext = try ChaChaPoly.open(sealedBox, using: derivedKey, authenticating: aad)
        } catch {
            throw AutomationNotificationCryptoError.decryptionFailed
        }

        let decoded: NotificationPlaintext
        do {
            decoded = try JSONDecoder().decode(NotificationPlaintext.self, from: plaintext)
        } catch {
            throw AutomationNotificationCryptoError.plaintextInvalid
        }

        let title = decoded.title.trimmingCharacters(in: .whitespacesAndNewlines)
        let body = decoded.body.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !title.isEmpty, !body.isEmpty else {
            throw AutomationNotificationCryptoError.plaintextInvalid
        }

        return AutomationNotificationContent(title: title, body: body)
    }

    public static func resolveVisibleContent(
        from userInfo: [AnyHashable: Any],
        timeoutNanoseconds: UInt64 = 1_500_000_000,
        decryptor: (@Sendable (AutomationEncryptedNotificationEnvelope, AutomationNotificationDecryptionMaterial) async throws -> AutomationNotificationContent)? = nil
    ) async -> AutomationNotificationContent {
        let envelope: AutomationEncryptedNotificationEnvelope
        do {
            envelope = try encryptedEnvelope(from: userInfo)
        } catch {
            return .fallback
        }

        let material: AutomationNotificationDecryptionMaterial
        do {
            guard let loaded = try decryptionMaterial() else {
                return .fallback
            }
            material = loaded
        } catch {
            return .fallback
        }

        let decryptOperation = decryptor ?? { envelope, material in
            try decrypt(envelope: envelope, material: material)
        }

        return await withTaskGroup(of: AutomationNotificationContent?.self) { group in
            group.addTask {
                try? await decryptOperation(envelope, material)
            }
            group.addTask {
                if timeoutNanoseconds > 0 {
                    try? await Task.sleep(nanoseconds: timeoutNanoseconds)
                }
                return nil
            }

            let resolved = await group.next() ?? nil
            group.cancelAll()
            return resolved ?? .fallback
        }
    }

    private static func deriveNotificationSymmetricKey(
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

    private static func decodePrivateKeyData(base64Encoded: String) throws -> Data {
        guard let privateKeyData = Data(base64Encoded: base64Encoded), privateKeyData.count == 32 else {
            throw AutomationNotificationCryptoError.invalidKeyMaterial
        }
        return privateKeyData
    }

    private static func normalizeUserInfo(_ userInfo: [AnyHashable: Any]) -> [String: Any] {
        userInfo.reduce(into: [String: Any]()) { partialResult, entry in
            if let key = entry.key as? String {
                partialResult[key] = entry.value
            }
        }
    }

    private static func loadOrCreateStoredKeyMaterial() throws -> StoredKeyMaterial {
        do {
            if let stored = try loadStoredKeyMaterial() {
                _ = try decodePrivateKeyData(base64Encoded: stored.privateKeyBase64)
                return stored
            }
        } catch AutomationNotificationCryptoError.invalidKeyMaterial {
            // Regenerate key material if a stale/corrupt payload was found.
        } catch {
            throw error
        }

        let generated = StoredKeyMaterial(
            deviceID: UUID().uuidString.lowercased(),
            privateKeyBase64: Curve25519.KeyAgreement.PrivateKey().rawRepresentation.base64EncodedString()
        )
        try store(storedKeyMaterial: generated)
        return generated
    }

    private static func loadStoredKeyMaterial() throws -> StoredKeyMaterial? {
        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: keychainService,
            kSecAttrAccount as String: keychainAccount,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]

        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)
        if status == errSecItemNotFound {
            return nil
        }
        guard status == errSecSuccess else {
            throw AutomationNotificationCryptoError.keychain(status)
        }
        guard let data = item as? Data else {
            throw AutomationNotificationCryptoError.invalidKeyMaterial
        }

        do {
            return try JSONDecoder().decode(StoredKeyMaterial.self, from: data)
        } catch {
            // Reset corrupt key material to avoid repeated decrypt failures.
            _ = SecItemDelete(query as CFDictionary)
            throw AutomationNotificationCryptoError.invalidKeyMaterial
        }
    }

    private static func store(storedKeyMaterial: StoredKeyMaterial) throws {
        let encoded = try JSONEncoder().encode(storedKeyMaterial)

        let attributes: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: keychainService,
            kSecAttrAccount as String: keychainAccount,
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly,
            kSecValueData as String: encoded,
        ]

        let addStatus = SecItemAdd(attributes as CFDictionary, nil)
        if addStatus == errSecSuccess {
            return
        }
        guard addStatus == errSecDuplicateItem else {
            throw AutomationNotificationCryptoError.keychain(addStatus)
        }

        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: keychainService,
            kSecAttrAccount as String: keychainAccount,
        ]
        let update: [String: Any] = [
            kSecValueData as String: encoded,
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly,
        ]
        let updateStatus = SecItemUpdate(query as CFDictionary, update as CFDictionary)
        guard updateStatus == errSecSuccess else {
            throw AutomationNotificationCryptoError.keychain(updateStatus)
        }
    }
}
