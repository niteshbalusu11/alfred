import XCTest
@testable import alfred

final class KittenEnglishPhonemizerTests: XCTestCase {
    func testDictionaryLookupPhonemizesKnownWords() async throws {
        let phonemizer = KittenEnglishPhonemizer(
            pronunciationDictionary: [
                "hello": "həˈloʊ",
                "world": "wɝld",
            ]
        )

        let result = try await phonemizer.phonemize("Hello world!")
        XCTAssertEqual(result, "həˈloʊ wɝld!")
    }

    func testNumberTokensAreExpandedAndPhonemized() async throws {
        let phonemizer = KittenEnglishPhonemizer(
            pronunciationDictionary: [
                "two": "tu",
            ]
        )

        let result = try await phonemizer.phonemize("2")
        XCTAssertEqual(result, "tu")
    }

    func testPossessiveSuffixFallsBackToBaseWordPlusZ() async throws {
        let phonemizer = KittenEnglishPhonemizer(
            pronunciationDictionary: [
                "alfred": "ˈælfrəd",
            ]
        )

        let result = try await phonemizer.phonemize("Alfred's")
        XCTAssertEqual(result, "ˈælfrədz")
    }

    func testDottedInitialismPhonemizesAsLetterNames() async throws {
        let phonemizer = KittenEnglishPhonemizer(pronunciationDictionary: [:])
        let result = try await phonemizer.phonemize("D.C.")
        XCTAssertEqual(result, "dˈiː sˈiː")
    }

    func testMissingDictionaryProperNounsCanUseCustomOverrides() async throws {
        let phonemizer = KittenEnglishPhonemizer(pronunciationDictionary: [:])
        let result = try await phonemizer.phonemize("Washington")
        XCTAssertEqual(result, "wˈɑːʃɪŋtən")
    }

    func testWashingtonDCPhraseUsesOverridesAndInitialismPhonemes() async throws {
        let phonemizer = KittenEnglishPhonemizer(pronunciationDictionary: [:])
        let result = try await phonemizer.phonemize("Washington, D.C.")
        XCTAssertEqual(result, "wˈɑːʃɪŋtən, dˈiː sˈiː")
    }

    func testNewDelhiPhraseIsPronouncedWithDelhiOverride() async throws {
        let phonemizer = KittenEnglishPhonemizer(
            pronunciationDictionary: [
                "new": "nˈuː",
            ]
        )
        let result = try await phonemizer.phonemize("New Delhi")
        XCTAssertEqual(result, "nˈuː dˈɛli")
    }

    func testOutOfDictionaryWordsUseHeuristicFallbackPronunciation() async throws {
        let phonemizer = KittenEnglishPhonemizer(pronunciationDictionary: [:])
        let result = try await phonemizer.phonemize("zorbak")
        XCTAssertNotEqual(result, "zorbak")
        XCTAssertFalse(result.isEmpty)
    }

    func testLowercaseCityInitialismsUseLetterPronunciation() async throws {
        let phonemizer = KittenEnglishPhonemizer(pronunciationDictionary: [:])
        let result = try await phonemizer.phonemize("sfo la")
        XCTAssertEqual(result, "ˈɛs ˈɛf ˈoʊ ˈɛl ˈeɪ")
    }

    func testContractionImUsesCustomPronunciation() async throws {
        let phonemizer = KittenEnglishPhonemizer(pronunciationDictionary: [:])
        let result = try await phonemizer.phonemize("I'm")
        XCTAssertEqual(result, "ˈaɪ ˈæm")
    }

    func testTimezoneAbbreviationsUseLetterPronunciation() async throws {
        let phonemizer = KittenEnglishPhonemizer(pronunciationDictionary: [:])
        let result = try await phonemizer.phonemize("UTC PST")
        XCTAssertEqual(result, "jˈuː tˈiː sˈiː pˈiː ˈɛs tˈiː")
    }

    func testCityOverridesIncludeIndiaAndUSCities() async throws {
        let phonemizer = KittenEnglishPhonemizer(pronunciationDictionary: [:])
        let result = try await phonemizer.phonemize("Mumbai Chennai Kolkata Philadelphia")
        XCTAssertEqual(result, "mʊmbˈaɪ tʃɛnˈaɪ koʊlkˈɑːtə fɪlədˈɛlfiə")
    }
}
