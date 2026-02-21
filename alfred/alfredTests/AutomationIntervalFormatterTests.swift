import AlfredAPIClient
import Testing
@testable import alfred

struct AutomationScheduleFormatterTests {
    @Test
    func formatsDailySchedule() {
        let schedule = AutomationSchedule(
            scheduleType: .daily,
            timeZone: "UTC",
            localTime: "09:00"
        )
        #expect(AutomationScheduleFormatter.label(for: schedule) == "Daily at 09:00")
    }

    @Test
    func formatsWeeklyMonthlyAndAnnualSchedules() {
        #expect(
            AutomationScheduleFormatter.label(
                for: AutomationSchedule(scheduleType: .weekly, timeZone: "UTC", localTime: "10:30")
            ) == "Weekly at 10:30"
        )
        #expect(
            AutomationScheduleFormatter.label(
                for: AutomationSchedule(scheduleType: .monthly, timeZone: "UTC", localTime: "08:15")
            ) == "Monthly at 08:15"
        )
        #expect(
            AutomationScheduleFormatter.label(
                for: AutomationSchedule(scheduleType: .annually, timeZone: "UTC", localTime: "07:45")
            ) == "Annually at 07:45"
        )
    }
}
