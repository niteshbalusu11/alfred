import Foundation

public enum APNSEnvironment: String, Codable, Sendable {
    case sandbox
    case production
}

public struct RegisterDeviceRequest: Codable, Sendable {
    public let deviceId: String
    public let apnsToken: String
    public let environment: APNSEnvironment

    enum CodingKeys: String, CodingKey {
        case deviceId = "device_id"
        case apnsToken = "apns_token"
        case environment
    }

    public init(deviceId: String, apnsToken: String, environment: APNSEnvironment) {
        self.deviceId = deviceId
        self.apnsToken = apnsToken
        self.environment = environment
    }
}

public struct SendTestNotificationRequest: Codable, Sendable {
    public let title: String?
    public let body: String?

    public init(title: String? = nil, body: String? = nil) {
        self.title = title
        self.body = body
    }
}

public struct SendTestNotificationResponse: Codable, Sendable {
    public let queuedJobId: String
    public let status: String

    enum CodingKeys: String, CodingKey {
        case queuedJobId = "queued_job_id"
        case status
    }
}

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

public enum AssistantQueryCapability: String, Codable, Sendable {
    case meetingsToday = "meetings_today"
}

public struct AssistantMeetingsTodayPayload: Codable, Sendable {
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
    public let payload: AssistantMeetingsTodayPayload

    enum CodingKeys: String, CodingKey {
        case sessionId = "session_id"
        case capability
        case displayText = "display_text"
        case payload
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

public struct StartGoogleConnectRequest: Codable, Sendable {
    public let redirectURI: String

    enum CodingKeys: String, CodingKey {
        case redirectURI = "redirect_uri"
    }

    public init(redirectURI: String) {
        self.redirectURI = redirectURI
    }
}

public struct StartGoogleConnectResponse: Codable, Sendable {
    public let authURL: String
    public let state: String

    enum CodingKeys: String, CodingKey {
        case authURL = "auth_url"
        case state
    }
}

public struct CompleteGoogleConnectRequest: Codable, Sendable {
    public let code: String?
    public let state: String
    public let error: String?
    public let errorDescription: String?

    enum CodingKeys: String, CodingKey {
        case code
        case state
        case error
        case errorDescription = "error_description"
    }

    public init(code: String? = nil, state: String, error: String? = nil, errorDescription: String? = nil) {
        self.code = code
        self.state = state
        self.error = error
        self.errorDescription = errorDescription
    }
}

public enum ConnectorStatus: String, Codable, Sendable {
    case active = "ACTIVE"
    case revoked = "REVOKED"
}

public struct CompleteGoogleConnectResponse: Codable, Sendable {
    public let connectorId: String
    public let status: ConnectorStatus
    public let grantedScopes: [String]

    enum CodingKeys: String, CodingKey {
        case connectorId = "connector_id"
        case status
        case grantedScopes = "granted_scopes"
    }
}

public struct RevokeConnectorResponse: Codable, Sendable {
    public let status: ConnectorStatus
}

public struct Preferences: Codable, Sendable {
    public let meetingReminderMinutes: Int
    public let morningBriefLocalTime: String
    public let quietHoursStart: String
    public let quietHoursEnd: String
    public let timeZone: String
    public let highRiskRequiresConfirm: Bool

    enum CodingKeys: String, CodingKey {
        case meetingReminderMinutes = "meeting_reminder_minutes"
        case morningBriefLocalTime = "morning_brief_local_time"
        case quietHoursStart = "quiet_hours_start"
        case quietHoursEnd = "quiet_hours_end"
        case timeZone = "time_zone"
        case highRiskRequiresConfirm = "high_risk_requires_confirm"
    }

    public init(
        meetingReminderMinutes: Int,
        morningBriefLocalTime: String,
        quietHoursStart: String,
        quietHoursEnd: String,
        timeZone: String,
        highRiskRequiresConfirm: Bool
    ) {
        self.meetingReminderMinutes = meetingReminderMinutes
        self.morningBriefLocalTime = morningBriefLocalTime
        self.quietHoursStart = quietHoursStart
        self.quietHoursEnd = quietHoursEnd
        self.timeZone = timeZone
        self.highRiskRequiresConfirm = highRiskRequiresConfirm
    }
}

public typealias UpdatePreferencesRequest = Preferences

public struct AuditEvent: Codable, Sendable {
    public let id: String
    public let timestamp: Date
    public let eventType: String
    public let connector: String?
    public let result: String
    public let metadata: [String: StringOrNumberOrBool]

    enum CodingKeys: String, CodingKey {
        case id
        case timestamp
        case eventType = "event_type"
        case connector
        case result
        case metadata
    }
}

public struct ListAuditEventsResponse: Codable, Sendable {
    public let items: [AuditEvent]
    public let nextCursor: String?

    enum CodingKeys: String, CodingKey {
        case items
        case nextCursor = "next_cursor"
    }
}

public struct DeleteAllResponse: Codable, Sendable {
    public let requestId: String
    public let status: String

    enum CodingKeys: String, CodingKey {
        case requestId = "request_id"
        case status
    }
}

public struct OkResponse: Codable, Sendable {
    public let ok: Bool
}

public struct APIErrorEnvelope: Codable, Sendable {
    public let error: APIErrorBody
}

public struct APIErrorBody: Codable, Sendable {
    public let code: String
    public let message: String
}

public enum StringOrNumberOrBool: Codable, Sendable {
    case string(String)
    case int(Int)
    case double(Double)
    case bool(Bool)

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let value = try? container.decode(String.self) {
            self = .string(value)
            return
        }
        if let value = try? container.decode(Int.self) {
            self = .int(value)
            return
        }
        if let value = try? container.decode(Double.self) {
            self = .double(value)
            return
        }
        if let value = try? container.decode(Bool.self) {
            self = .bool(value)
            return
        }
        throw DecodingError.typeMismatch(
            StringOrNumberOrBool.self,
            DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Unsupported metadata value")
        )
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .string(let value):
            try container.encode(value)
        case .int(let value):
            try container.encode(value)
        case .double(let value):
            try container.encode(value)
        case .bool(let value):
            try container.encode(value)
        }
    }
}
