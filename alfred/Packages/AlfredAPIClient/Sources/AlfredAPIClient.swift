import Foundation

public typealias AccessTokenProvider = @Sendable () async throws -> String?

public enum AlfredAPIClientError: Error, Sendable {
    case invalidURL
    case invalidResponse
    case unauthorized
    case serverError(statusCode: Int, code: String?, message: String?)
    case decodingError
}

public final class AlfredAPIClient: Sendable {
    private static let pathComponentAllowedCharacters: CharacterSet = {
        var allowed = CharacterSet.urlPathAllowed
        allowed.remove(charactersIn: "/")
        return allowed
    }()

    private let baseURL: URL
    private let session: URLSession
    private let tokenProvider: AccessTokenProvider?
    private let jsonDecoder: JSONDecoder
    private let jsonEncoder: JSONEncoder

    public init(
        baseURL: URL,
        session: URLSession = .shared,
        tokenProvider: AccessTokenProvider? = nil
    ) {
        self.baseURL = baseURL
        self.session = session
        self.tokenProvider = tokenProvider

        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        self.jsonDecoder = decoder

        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        self.jsonEncoder = encoder
    }

    public func registerAPNSDevice(_ request: RegisterDeviceRequest) async throws -> OkResponse {
        try await send(
            method: "POST",
            path: "/v1/devices/apns",
            body: request,
            requiresAuth: true
        )
    }

    public func sendAPNSTestNotification(_ request: SendTestNotificationRequest) async throws -> SendTestNotificationResponse {
        try await send(
            method: "POST",
            path: "/v1/devices/apns/test",
            body: request,
            requiresAuth: true
        )
    }

    public func startGoogleOAuth(_ request: StartGoogleConnectRequest) async throws -> StartGoogleConnectResponse {
        try await send(
            method: "POST",
            path: "/v1/connectors/google/start",
            body: request,
            requiresAuth: true
        )
    }

    public func completeGoogleOAuth(_ request: CompleteGoogleConnectRequest) async throws -> CompleteGoogleConnectResponse {
        try await send(
            method: "POST",
            path: "/v1/connectors/google/callback",
            body: request,
            requiresAuth: true
        )
    }

    public func revokeConnector(connectorID: String) async throws -> RevokeConnectorResponse {
        guard let encodedConnectorID = connectorID.addingPercentEncoding(withAllowedCharacters: Self.pathComponentAllowedCharacters) else {
            throw AlfredAPIClientError.invalidURL
        }

        return try await send(
            method: "DELETE",
            path: "/v1/connectors/\(encodedConnectorID)",
            body: Optional<EmptyBody>.none,
            requiresAuth: true
        )
    }

    public func getPreferences() async throws -> Preferences {
        try await send(
            method: "GET",
            path: "/v1/preferences",
            body: Optional<EmptyBody>.none,
            requiresAuth: true
        )
    }

    public func updatePreferences(_ request: UpdatePreferencesRequest) async throws -> OkResponse {
        try await send(
            method: "PUT",
            path: "/v1/preferences",
            body: request,
            requiresAuth: true
        )
    }

    public func listAuditEvents(cursor: String? = nil) async throws -> ListAuditEventsResponse {
        var path = "/v1/audit-events"
        if let cursor, !cursor.isEmpty {
            let encoded = cursor.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? cursor
            path += "?cursor=\(encoded)"
        }
        return try await send(
            method: "GET",
            path: path,
            body: Optional<EmptyBody>.none,
            requiresAuth: true
        )
    }

    public func requestDeleteAll() async throws -> DeleteAllResponse {
        try await send(
            method: "POST",
            path: "/v1/privacy/delete-all",
            body: Optional<EmptyBody>.none,
            requiresAuth: true
        )
    }

    private func send<T: Decodable, U: Encodable>(
        method: String,
        path: String,
        body: U?,
        requiresAuth: Bool
    ) async throws -> T {
        guard let url = URL(string: path, relativeTo: baseURL) else {
            throw AlfredAPIClientError.invalidURL
        }

        var request = URLRequest(url: url)
        request.httpMethod = method
        request.setValue("application/json", forHTTPHeaderField: "Accept")

        if let body {
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
            request.httpBody = try jsonEncoder.encode(body)
        }

        if requiresAuth {
            guard let token = try await tokenProvider?(), !token.isEmpty else {
                throw AlfredAPIClientError.unauthorized
            }
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }

        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse else {
            throw AlfredAPIClientError.invalidResponse
        }

        switch http.statusCode {
        case 200..<300:
            do {
                return try jsonDecoder.decode(T.self, from: data)
            } catch {
                throw AlfredAPIClientError.decodingError
            }
        case 401:
            throw AlfredAPIClientError.unauthorized
        default:
            let envelope = try? jsonDecoder.decode(APIErrorEnvelope.self, from: data)
            throw AlfredAPIClientError.serverError(
                statusCode: http.statusCode,
                code: envelope?.error.code,
                message: envelope?.error.message
            )
        }
    }
}

private struct EmptyBody: Codable {}
