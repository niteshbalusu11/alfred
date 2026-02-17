import Foundation

public struct AssistantQueryRequest: Codable, Sendable {
    public let envelope: AssistantEncryptedRequestEnvelope
    public let sessionId: UUID?

    enum CodingKeys: String, CodingKey {
        case envelope
        case sessionId = "session_id"
    }

    public init(envelope: AssistantEncryptedRequestEnvelope, sessionId: UUID? = nil) {
        self.envelope = envelope
        self.sessionId = sessionId
    }
}

public struct AssistantEncryptedRequestEnvelope: Codable, Sendable {
    public let version: String
    public let algorithm: String
    public let keyId: String
    public let requestId: String
    public let clientEphemeralPublicKey: String
    public let nonce: String
    public let ciphertext: String

    enum CodingKeys: String, CodingKey {
        case version
        case algorithm
        case keyId = "key_id"
        case requestId = "request_id"
        case clientEphemeralPublicKey = "client_ephemeral_public_key"
        case nonce
        case ciphertext
    }
}

public struct AssistantEncryptedResponseEnvelope: Codable, Sendable {
    public let version: String
    public let algorithm: String
    public let keyId: String
    public let requestId: String
    public let nonce: String
    public let ciphertext: String

    enum CodingKeys: String, CodingKey {
        case version
        case algorithm
        case keyId = "key_id"
        case requestId = "request_id"
        case nonce
        case ciphertext
    }
}

public struct AssistantQueryResponse: Codable, Sendable {
    public let sessionId: UUID
    public let envelope: AssistantEncryptedResponseEnvelope

    enum CodingKeys: String, CodingKey {
        case sessionId = "session_id"
        case envelope
    }
}

public struct AssistantPlaintextQueryRequest: Codable, Sendable {
    public let query: String
    public let sessionId: UUID?

    enum CodingKeys: String, CodingKey {
        case query
        case sessionId = "session_id"
    }

    public init(query: String, sessionId: UUID? = nil) {
        self.query = query
        self.sessionId = sessionId
    }
}

public enum AssistantQueryCapability: String, Codable, Sendable, Equatable {
    case meetingsToday = "meetings_today"
    case calendarLookup = "calendar_lookup"
    case emailLookup = "email_lookup"
    case generalChat = "general_chat"
    case mixed = "mixed"
}

public enum AssistantResponsePartType: String, Codable, Sendable, Equatable {
    case chatText = "chat_text"
    case toolSummary = "tool_summary"
}

public struct AssistantResponsePart: Codable, Sendable, Equatable {
    public let type: AssistantResponsePartType
    public let text: String?
    public let capability: AssistantQueryCapability?
    public let payload: AssistantStructuredPayload?

    enum CodingKeys: String, CodingKey {
        case type
        case text
        case capability
        case payload
    }

    public init(
        type: AssistantResponsePartType,
        text: String? = nil,
        capability: AssistantQueryCapability? = nil,
        payload: AssistantStructuredPayload? = nil
    ) {
        self.type = type
        self.text = text
        self.capability = capability
        self.payload = payload
    }
}

public struct AssistantStructuredPayload: Codable, Sendable, Equatable {
    public let title: String
    public let summary: String
    public let keyPoints: [String]
    public let followUps: [String]

    enum CodingKeys: String, CodingKey {
        case title
        case summary
        case keyPoints = "key_points"
        case followUps = "follow_ups"
    }
}

public struct AssistantPlaintextQueryResponse: Codable, Sendable {
    public let sessionId: UUID
    public let capability: AssistantQueryCapability
    public let displayText: String
    public let payload: AssistantStructuredPayload
    public let responseParts: [AssistantResponsePart]

    enum CodingKeys: String, CodingKey {
        case sessionId = "session_id"
        case capability
        case displayText = "display_text"
        case payload
        case responseParts = "response_parts"
    }

    public init(
        sessionId: UUID,
        capability: AssistantQueryCapability,
        displayText: String,
        payload: AssistantStructuredPayload,
        responseParts: [AssistantResponsePart]
    ) {
        self.sessionId = sessionId
        self.capability = capability
        self.displayText = displayText
        self.payload = payload
        self.responseParts = responseParts
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let sessionId = try container.decode(UUID.self, forKey: .sessionId)

        let responseParts = try container.decodeIfPresent([AssistantResponsePart].self, forKey: .responseParts)
        if let responseParts {
            let decodedCapability =
                try container.decodeIfPresent(AssistantQueryCapability.self, forKey: .capability)
                ?? responseParts.compactMap(\.capability).first
                ?? .generalChat
            let decodedPayload =
                try container.decodeIfPresent(AssistantStructuredPayload.self, forKey: .payload)
                ?? responseParts.compactMap(\.payload).first
                ?? Self.fallbackPayload(displayText: "")

            let displayText = try container.decodeIfPresent(String.self, forKey: .displayText)
                ?? responseParts
                .first(where: { $0.type == .chatText })
                .flatMap(\.text)
                ?? decodedPayload.summary

            self.sessionId = sessionId
            self.capability = decodedCapability
            self.displayText = displayText
            self.payload = decodedPayload
            self.responseParts = Self.normalizeResponseParts(
                responseParts,
                capability: decodedCapability,
                displayText: displayText
            )
            return
        }

        let capability = try container.decode(AssistantQueryCapability.self, forKey: .capability)
        let displayText = try container.decode(String.self, forKey: .displayText)
        let payload = try container.decode(AssistantStructuredPayload.self, forKey: .payload)

        self.sessionId = sessionId
        self.capability = capability
        self.displayText = displayText
        self.payload = payload
        self.responseParts = Self.legacyResponseParts(
            capability: capability,
            displayText: displayText,
            payload: payload
        )
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(sessionId, forKey: .sessionId)
        try container.encode(capability, forKey: .capability)
        try container.encode(displayText, forKey: .displayText)
        try container.encode(payload, forKey: .payload)
        if !responseParts.isEmpty {
            try container.encode(responseParts, forKey: .responseParts)
        }
    }

    private static func normalizeResponseParts(
        _ parts: [AssistantResponsePart],
        capability: AssistantQueryCapability,
        displayText: String
    ) -> [AssistantResponsePart] {
        var normalized = parts
        let hasChatPart = normalized.contains {
            $0.type == .chatText
                && !($0.text?.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ?? true)
        }
        if !hasChatPart {
            normalized.insert(
                AssistantResponsePart(type: .chatText, text: displayText),
                at: 0
            )
        }
        if normalized.isEmpty {
            return [AssistantResponsePart(type: .chatText, text: displayText)]
        }

        return normalized.map { part in
            if part.type == .toolSummary && part.capability == nil {
                return AssistantResponsePart(
                    type: .toolSummary,
                    text: part.text,
                    capability: capability,
                    payload: part.payload
                )
            }
            return part
        }
    }

    private static func legacyResponseParts(
        capability: AssistantQueryCapability,
        displayText: String,
        payload: AssistantStructuredPayload
    ) -> [AssistantResponsePart] {
        var parts = [AssistantResponsePart(type: .chatText, text: displayText)]
        if capability != .generalChat {
            parts.append(
                AssistantResponsePart(
                    type: .toolSummary,
                    capability: capability,
                    payload: payload
                )
            )
        }
        return parts
    }

    private static func fallbackPayload(displayText: String) -> AssistantStructuredPayload {
        AssistantStructuredPayload(
            title: "Assistant response",
            summary: displayText,
            keyPoints: [],
            followUps: []
        )
    }
}

public struct AssistantAttestedKeyRequest: Codable, Sendable {
    public let challengeNonce: String
    public let issuedAt: Int64
    public let expiresAt: Int64
    public let requestId: String

    enum CodingKeys: String, CodingKey {
        case challengeNonce = "challenge_nonce"
        case issuedAt = "issued_at"
        case expiresAt = "expires_at"
        case requestId = "request_id"
    }

    public init(challengeNonce: String, issuedAt: Int64, expiresAt: Int64, requestId: String) {
        self.challengeNonce = challengeNonce
        self.issuedAt = issuedAt
        self.expiresAt = expiresAt
        self.requestId = requestId
    }
}

public struct AssistantAttestedKeyAttestation: Codable, Sendable {
    public let runtime: String
    public let measurement: String
    public let challengeNonce: String
    public let issuedAt: Int64
    public let expiresAt: Int64
    public let requestId: String
    public let evidenceIssuedAt: Int64
    public let signature: String?

    enum CodingKeys: String, CodingKey {
        case runtime
        case measurement
        case challengeNonce = "challenge_nonce"
        case issuedAt = "issued_at"
        case expiresAt = "expires_at"
        case requestId = "request_id"
        case evidenceIssuedAt = "evidence_issued_at"
        case signature
    }
}

public struct AssistantAttestedKeyResponse: Codable, Sendable {
    public let keyId: String
    public let algorithm: String
    public let publicKey: String
    public let keyExpiresAt: Int64
    public let attestation: AssistantAttestedKeyAttestation

    enum CodingKeys: String, CodingKey {
        case keyId = "key_id"
        case algorithm
        case publicKey = "public_key"
        case keyExpiresAt = "key_expires_at"
        case attestation
    }
}
