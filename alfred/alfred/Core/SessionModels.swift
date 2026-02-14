import Foundation

struct StoredSession: Codable, Equatable, Sendable {
    let accessToken: String
    let refreshToken: String
    let expiresAt: Date

    func isValid(at date: Date) -> Bool {
        expiresAt > date
    }
}
