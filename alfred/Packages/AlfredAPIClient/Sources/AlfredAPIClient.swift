import Foundation

public typealias AccessTokenProvider = @Sendable () async throws -> String?

public enum AlfredAPIClientError: Error, Sendable {
    case invalidURL
    case invalidResponse
    case unauthorized
    case serverError(statusCode: Int, code: String?, message: String?)
    case decodingError
    case assistantAttestationFailed(reason: String)
    case assistantEncryptionFailed(reason: String)
    case assistantDecryptionFailed(reason: String)
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

    public func queryAssistant(_ request: AssistantQueryRequest) async throws -> AssistantQueryResponse {
        try await send(
            method: "POST",
            path: "/v1/assistant/query",
            body: request,
            requiresAuth: true
        )
    }

    public func deleteAssistantSession(sessionID: UUID) async throws -> OkResponse {
        try await send(
            method: "DELETE",
            path: "/v1/assistant/sessions/\(sessionID.uuidString.lowercased())",
            body: Optional<EmptyBody>.none,
            requiresAuth: true
        )
    }

    public func deleteAllAssistantSessions() async throws -> OkResponse {
        try await send(
            method: "DELETE",
            path: "/v1/assistant/sessions",
            body: Optional<EmptyBody>.none,
            requiresAuth: true
        )
    }

    public func fetchAssistantAttestedKey(_ request: AssistantAttestedKeyRequest) async throws -> AssistantAttestedKeyResponse {
        try await send(
            method: "POST",
            path: "/v1/assistant/attested-key",
            body: request,
            requiresAuth: true
        )
    }

    public func queryAssistantEncrypted(
        query: String,
        sessionId: UUID? = nil,
        attestationConfig: AssistantAttestationVerificationConfig
    ) async throws -> AssistantPlaintextQueryResponse {
        let challengeNonce = UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased()
        let requestID = UUID().uuidString
        let issuedAt = Int64(Date().timeIntervalSince1970)
        let expiresAt = issuedAt + Int64(attestationConfig.challengeWindowSeconds)
        let keyResponse = try await fetchAssistantAttestedKey(
            AssistantAttestedKeyRequest(
                challengeNonce: challengeNonce,
                issuedAt: issuedAt,
                expiresAt: expiresAt,
                requestId: requestID
            )
        )

        try AssistantEnvelopeCrypto.verifyAttestedKeyResponse(
            keyResponse,
            expectedChallengeNonce: challengeNonce,
            expectedRequestID: requestID,
            config: attestationConfig
        )

        let plaintextRequest = AssistantPlaintextQueryRequest(query: query, sessionId: sessionId)
        let encryptedPayload = try AssistantEnvelopeCrypto.encryptRequest(
            plaintextRequest: plaintextRequest,
            requestID: requestID,
            attestedKey: keyResponse
        )
        let apiResponse = try await queryAssistant(
            AssistantQueryRequest(envelope: encryptedPayload.envelope, sessionId: sessionId)
        )

        guard apiResponse.envelope.requestId == requestID else {
            throw AlfredAPIClientError.assistantDecryptionFailed(reason: "response request_id mismatch")
        }

        return try AssistantEnvelopeCrypto.decryptResponse(
            envelope: apiResponse.envelope,
            requestID: requestID,
            clientEphemeralPrivateKey: encryptedPayload.clientEphemeralPrivateKey,
            attestedKey: keyResponse
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

    public func listConnectors() async throws -> ListConnectorsResponse {
        try await send(
            method: "GET",
            path: "/v1/connectors",
            body: Optional<EmptyBody>.none,
            requiresAuth: true
        )
    }

    public func listAutomations(limit: Int? = nil) async throws -> ListAutomationsResponse {
        var path = "/v1/automations"
        if let limit {
            path += "?limit=\(limit)"
        }

        return try await send(
            method: "GET",
            path: path,
            body: Optional<EmptyBody>.none,
            requiresAuth: true
        )
    }

    public func createAutomation(_ request: CreateAutomationRequest) async throws -> AutomationRuleSummary {
        try await send(
            method: "POST",
            path: "/v1/automations",
            body: request,
            requiresAuth: true
        )
    }

    public func createAutomationEncrypted(
        title: String,
        schedule: AutomationSchedule,
        prompt: String,
        attestationConfig: AssistantAttestationVerificationConfig
    ) async throws -> AutomationRuleSummary {
        let encryptedEnvelope = try await encryptAutomationPromptEnvelope(
            prompt: prompt,
            attestationConfig: attestationConfig
        )

        return try await createAutomation(
            CreateAutomationRequest(
                title: title,
                schedule: schedule,
                promptEnvelope: encryptedEnvelope
            )
        )
    }

    public func updateAutomation(
        ruleID: UUID,
        request: UpdateAutomationRequest
    ) async throws -> AutomationRuleSummary {
        try await send(
            method: "PATCH",
            path: "/v1/automations/\(ruleID.uuidString.lowercased())",
            body: request,
            requiresAuth: true
        )
    }

    public func updateAutomationEncrypted(
        ruleID: UUID,
        title: String? = nil,
        schedule: AutomationSchedule? = nil,
        prompt: String? = nil,
        status: AutomationStatus? = nil,
        attestationConfig: AssistantAttestationVerificationConfig
    ) async throws -> AutomationRuleSummary {
        let promptEnvelope: AssistantEncryptedRequestEnvelope?
        if let prompt {
            let trimmed = prompt.trimmingCharacters(in: .whitespacesAndNewlines)
            if trimmed.isEmpty {
                promptEnvelope = nil
            } else {
                promptEnvelope = try await encryptAutomationPromptEnvelope(
                    prompt: trimmed,
                    attestationConfig: attestationConfig
                )
            }
        } else {
            promptEnvelope = nil
        }

        return try await updateAutomation(
            ruleID: ruleID,
            request: UpdateAutomationRequest(
                title: title,
                schedule: schedule,
                promptEnvelope: promptEnvelope,
                status: status
            )
        )
    }

    public func deleteAutomation(ruleID: UUID) async throws -> OkResponse {
        try await send(
            method: "DELETE",
            path: "/v1/automations/\(ruleID.uuidString.lowercased())",
            body: Optional<EmptyBody>.none,
            requiresAuth: true
        )
    }

    public func triggerAutomationDebugRun(ruleID: UUID) async throws -> TriggerAutomationDebugRunResponse {
        try await send(
            method: "POST",
            path: "/v1/automations/\(ruleID.uuidString.lowercased())/debug/run",
            body: Optional<EmptyBody>.none,
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

    private func encryptAutomationPromptEnvelope(
        prompt: String,
        attestationConfig: AssistantAttestationVerificationConfig
    ) async throws -> AssistantEncryptedRequestEnvelope {
        let challengeNonce = UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased()
        let requestID = UUID().uuidString
        let issuedAt = Int64(Date().timeIntervalSince1970)
        let expiresAt = issuedAt + Int64(attestationConfig.challengeWindowSeconds)
        let keyResponse = try await fetchAssistantAttestedKey(
            AssistantAttestedKeyRequest(
                challengeNonce: challengeNonce,
                issuedAt: issuedAt,
                expiresAt: expiresAt,
                requestId: requestID
            )
        )

        try AssistantEnvelopeCrypto.verifyAttestedKeyResponse(
            keyResponse,
            expectedChallengeNonce: challengeNonce,
            expectedRequestID: requestID,
            config: attestationConfig
        )

        let plaintextRequest = AssistantPlaintextQueryRequest(query: prompt, sessionId: nil)
        let encryptedPayload = try AssistantEnvelopeCrypto.encryptRequest(
            plaintextRequest: plaintextRequest,
            requestID: requestID,
            attestedKey: keyResponse
        )

        return encryptedPayload.envelope
    }
}

private struct EmptyBody: Codable {}
