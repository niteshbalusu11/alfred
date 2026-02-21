import Testing
@testable import alfred

struct AutomationIntervalFormatterTests {
    @Test
    func formatsMinutesIntervals() {
        #expect(AutomationIntervalFormatter.label(for: 60) == "every 1m")
        #expect(AutomationIntervalFormatter.label(for: 900) == "every 15m")
    }

    @Test
    func formatsHoursAndDaysIntervals() {
        #expect(AutomationIntervalFormatter.label(for: 3_600) == "every 1h")
        #expect(AutomationIntervalFormatter.label(for: 86_400) == "every 1d")
    }

    @Test
    func formatsRawSecondsWhenNotDivisibleByMinute() {
        #expect(AutomationIntervalFormatter.label(for: 95) == "every 95s")
    }
}
