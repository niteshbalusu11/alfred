import XCTest
@testable import alfred

final class KittenSpeechTextNormalizerTests: XCTestCase {
    func testNormalizeExpandsUtcTimeWithSeconds() {
        let normalized = KittenSpeechTextNormalizer.normalize("The server restarts at 12:00:00 UTC.")

        XCTAssertEqual(normalized, "The server restarts at twelve o'clock U T C.")
    }

    func testNormalizeExpandsPercentAndDecimals() {
        let normalized = KittenSpeechTextNormalizer.normalize("Latency dropped 12.5% this week.")

        XCTAssertEqual(normalized, "Latency dropped twelve point five percent this week.")
    }

    func testNormalizeExpandsCurrencyAndIntegers() {
        let normalized = KittenSpeechTextNormalizer.normalize("Budget is $1,200 for 42 requests.")

        XCTAssertEqual(normalized, "Budget is one thousand two hundred dollars for forty-two requests.")
    }

    func testNormalizeStripsMarkdownFormattingArtifacts() {
        let normalized = KittenSpeechTextNormalizer.normalize("Use [`tool`](https://example.com) and `run --fast`.")

        XCTAssertEqual(normalized, "Use tool and run --fast.")
    }
}
