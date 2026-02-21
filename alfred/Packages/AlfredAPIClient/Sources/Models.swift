import Foundation

public enum APNSEnvironment: String, Codable, Sendable {
    case sandbox
    case production
}

public struct RegisterDeviceRequest: Codable, Sendable {
    public let deviceId: String
    public let apnsToken: String
    public let environment: APNSEnvironment
    public let notificationKeyAlgorithm: String?
    public let notificationPublicKey: String?

    enum CodingKeys: String, CodingKey {
        case deviceId = "device_id"
        case apnsToken = "apns_token"
        case environment
        case notificationKeyAlgorithm = "notification_key_algorithm"
        case notificationPublicKey = "notification_public_key"
    }

    public init(
        deviceId: String,
        apnsToken: String,
        environment: APNSEnvironment,
        notificationKeyAlgorithm: String? = nil,
        notificationPublicKey: String? = nil
    ) {
        self.deviceId = deviceId
        self.apnsToken = apnsToken
        self.environment = environment
        self.notificationKeyAlgorithm = notificationKeyAlgorithm
        self.notificationPublicKey = notificationPublicKey
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

public struct ConnectorSummary: Codable, Sendable {
    public let connectorId: String
    public let provider: String
    public let status: ConnectorStatus

    enum CodingKeys: String, CodingKey {
        case connectorId = "connector_id"
        case provider
        case status
    }
}

public struct ListConnectorsResponse: Codable, Sendable {
    public let items: [ConnectorSummary]
}

public struct CreateAutomationRequest: Codable, Sendable {
    public let title: String
    public let schedule: AutomationSchedule
    public let promptEnvelope: AssistantEncryptedRequestEnvelope

    enum CodingKeys: String, CodingKey {
        case title
        case schedule
        case promptEnvelope = "prompt_envelope"
    }

    public init(
        title: String,
        schedule: AutomationSchedule,
        promptEnvelope: AssistantEncryptedRequestEnvelope
    ) {
        self.title = title
        self.schedule = schedule
        self.promptEnvelope = promptEnvelope
    }
}

public struct AutomationSchedule: Codable, Sendable {
    public let scheduleType: AutomationScheduleType
    public let timeZone: String
    public let localTime: String

    enum CodingKeys: String, CodingKey {
        case scheduleType = "schedule_type"
        case timeZone = "time_zone"
        case localTime = "local_time"
    }

    public init(scheduleType: AutomationScheduleType, timeZone: String, localTime: String) {
        self.scheduleType = scheduleType
        self.timeZone = timeZone
        self.localTime = localTime
    }
}

public enum AutomationStatus: String, Codable, Sendable {
    case active = "ACTIVE"
    case paused = "PAUSED"
}

public struct UpdateAutomationRequest: Codable, Sendable {
    public let title: String?
    public let schedule: AutomationSchedule?
    public let promptEnvelope: AssistantEncryptedRequestEnvelope?
    public let status: AutomationStatus?

    enum CodingKeys: String, CodingKey {
        case title
        case schedule
        case promptEnvelope = "prompt_envelope"
        case status
    }

    public init(
        title: String? = nil,
        schedule: AutomationSchedule? = nil,
        promptEnvelope: AssistantEncryptedRequestEnvelope? = nil,
        status: AutomationStatus? = nil
    ) {
        self.title = title
        self.schedule = schedule
        self.promptEnvelope = promptEnvelope
        self.status = status
    }
}

public enum AutomationScheduleType: String, Codable, Sendable {
    case daily = "DAILY"
    case weekly = "WEEKLY"
    case monthly = "MONTHLY"
    case annually = "ANNUALLY"
}

public struct AutomationRuleSummary: Codable, Sendable {
    public let ruleId: UUID
    public let title: String
    public let status: AutomationStatus
    public let schedule: AutomationSchedule
    public let nextRunAt: Date
    public let lastRunAt: Date?
    public let promptSha256: String
    public let createdAt: Date
    public let updatedAt: Date

    enum CodingKeys: String, CodingKey {
        case ruleId = "rule_id"
        case title
        case status
        case schedule
        case nextRunAt = "next_run_at"
        case lastRunAt = "last_run_at"
        case promptSha256 = "prompt_sha256"
        case createdAt = "created_at"
        case updatedAt = "updated_at"
    }
}

public struct ListAutomationsResponse: Codable, Sendable {
    public let items: [AutomationRuleSummary]
}

public struct TriggerAutomationDebugRunResponse: Codable, Sendable {
    public let queuedJobId: String
    public let status: String

    enum CodingKeys: String, CodingKey {
        case queuedJobId = "queued_job_id"
        case status
    }
}

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
