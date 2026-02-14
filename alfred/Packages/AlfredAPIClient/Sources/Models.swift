import Foundation

public struct CreateSessionRequest: Codable, Sendable {
    public let appleIdentityToken: String
    public let deviceId: String

    enum CodingKeys: String, CodingKey {
        case appleIdentityToken = "apple_identity_token"
        case deviceId = "device_id"
    }

    public init(appleIdentityToken: String, deviceId: String) {
        self.appleIdentityToken = appleIdentityToken
        self.deviceId = deviceId
    }
}

public struct CreateSessionResponse: Codable, Sendable {
    public let accessToken: String
    public let refreshToken: String
    public let expiresIn: Int

    enum CodingKeys: String, CodingKey {
        case accessToken = "access_token"
        case refreshToken = "refresh_token"
        case expiresIn = "expires_in"
    }
}

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
    public let code: String
    public let state: String

    public init(code: String, state: String) {
        self.code = code
        self.state = state
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
    public let highRiskRequiresConfirm: Bool

    enum CodingKeys: String, CodingKey {
        case meetingReminderMinutes = "meeting_reminder_minutes"
        case morningBriefLocalTime = "morning_brief_local_time"
        case quietHoursStart = "quiet_hours_start"
        case quietHoursEnd = "quiet_hours_end"
        case highRiskRequiresConfirm = "high_risk_requires_confirm"
    }

    public init(
        meetingReminderMinutes: Int,
        morningBriefLocalTime: String,
        quietHoursStart: String,
        quietHoursEnd: String,
        highRiskRequiresConfirm: Bool
    ) {
        self.meetingReminderMinutes = meetingReminderMinutes
        self.morningBriefLocalTime = morningBriefLocalTime
        self.quietHoursStart = quietHoursStart
        self.quietHoursEnd = quietHoursEnd
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
