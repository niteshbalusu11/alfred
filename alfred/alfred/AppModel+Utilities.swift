import AlfredAPIClient
import Foundation

extension AppModel {
    static func errorMessage(from error: Error) -> String {
        if let clientError = error as? AlfredAPIClientError {
            switch clientError {
            case .invalidURL:
                return "API URL is invalid."
            case .invalidResponse:
                return "API returned an invalid response."
            case .unauthorized:
                return "Session is unauthorized. Please sign in again."
            case .serverError(let statusCode, let code, let message):
                let details = [code, message].compactMap { $0 }.joined(separator: " - ")
                if details.isEmpty {
                    return "Server error (\(statusCode))."
                }
                return "Server error (\(statusCode)): \(details)"
            case .decodingError:
                return "Failed to decode API response."
            }
        }

        return error.localizedDescription
    }

    func trimmedOrNil(_ value: String) -> String? {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }
}
