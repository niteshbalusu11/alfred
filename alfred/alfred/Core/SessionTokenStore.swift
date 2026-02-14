import Foundation
import Security

protocol SessionTokenStore: Sendable {
    func readSessionData() throws -> Data?
    func writeSessionData(_ data: Data) throws
    func clearSessionData() throws
}

enum SessionTokenStoreError: Error {
    case readFailed(OSStatus)
    case writeFailed(OSStatus)
    case deleteFailed(OSStatus)
}
