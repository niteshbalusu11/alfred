import AlfredAPIClient
import SwiftUI

struct AutomationsView: View {
    @ObservedObject var model: AppModel
    @State private var rules: [AutomationRuleSummary] = []
    @State private var localTitles: [UUID: String] = [:]
    @State private var isLoading = false
    @State private var loadErrorMessage: String?
    @State private var mutationErrorMessage: String?
    @State private var mutatingRuleIDs: Set<UUID> = []
    @State private var activeSheet: SheetRoute?

    private enum SheetRoute: String, Identifiable {
        case create

        var id: String { rawValue }
    }

    var body: some View {
        ScrollView {
            LazyVStack(spacing: AppTheme.Layout.sectionSpacing) {
                headerCard

                if let mutationErrorMessage {
                    AutomationCallout(
                        title: "Automation action failed",
                        message: mutationErrorMessage,
                        buttonTitle: "Dismiss"
                    ) {
                        self.mutationErrorMessage = nil
                    }
                }

                if isLoading && rules.isEmpty {
                    AutomationLoadingStateCard()
                } else if rules.isEmpty {
                    AutomationEmptyStateCard(
                        title: emptyStateTitle,
                        message: emptyStateMessage,
                        buttonTitle: "Create automation"
                    ) {
                        activeSheet = .create
                    }
                } else {
                    ForEach(rules, id: \.ruleId) { rule in
                        AutomationRuleCard(
                            title: title(for: rule),
                            rule: rule,
                            isMutating: mutatingRuleIDs.contains(rule.ruleId),
                            onTogglePause: { togglePause(for: rule) },
                            onDelete: { delete(rule: rule) }
                        )
                    }
                }
            }
            .padding(.horizontal, AppTheme.Layout.screenPadding)
            .padding(.vertical, AppTheme.Layout.sectionSpacing)
        }
        .appScreenBackground()
        .task {
            if rules.isEmpty {
                await loadAutomations()
            }
        }
        .sheet(item: $activeSheet) { _ in
            AutomationCreateSheet(
                defaultTimeZone: TimeZone.current.identifier,
                onSubmit: { payload in
                    try await createAutomation(payload)
                }
            )
        }
    }

    private var headerCard: some View {
        AppCard {
            AppSectionHeader("Automations", subtitle: "Scheduled prompts with private push delivery") {
                HStack(spacing: 8) {
                    AppStatusBadge(
                        title: statusBadge.title,
                        style: statusBadge.style
                    )

                    AutomationInlineButton(title: isLoading ? "Loading…" : "Refresh") {
                        Task {
                            await loadAutomations()
                        }
                    }
                    .disabled(isLoading)

                    AutomationInlineButton(title: "New") {
                        activeSheet = .create
                    }
                }
            }

            if let loadErrorMessage, !rules.isEmpty {
                AutomationCallout(
                    title: "Could not refresh automations",
                    message: loadErrorMessage,
                    buttonTitle: "Retry"
                ) {
                    Task {
                        await loadAutomations()
                    }
                }
            }
        }
    }

    private var statusBadge: (title: String, style: AppStatusBadge.Style) {
        if isLoading {
            return ("Loading", .warning)
        }
        if rules.isEmpty {
            return ("Empty", .neutral)
        }
        if loadErrorMessage != nil {
            return ("Stale", .warning)
        }
        return ("Updated", .success)
    }

    private var emptyStateTitle: String {
        if loadErrorMessage != nil {
            return "Unable to load automations"
        }
        return "No automations yet"
    }

    private var emptyStateMessage: String {
        if let loadErrorMessage {
            return loadErrorMessage
        }
        return "Create a periodic prompt and Alfred will run it for you automatically."
    }

    @MainActor
    private func loadAutomations() async {
        guard !isLoading else { return }
        isLoading = true
        defer { isLoading = false }

        do {
            let response = try await model.apiClient.listAutomations(limit: 100)
            rules = response.items.sorted(by: { $0.nextRunAt < $1.nextRunAt })
            loadErrorMessage = nil
        } catch {
            loadErrorMessage = AppModel.errorMessage(from: error)
        }
    }

    @MainActor
    private func createAutomation(_ payload: AutomationCreatePayload) async throws {
        let created = try await model.apiClient.createAutomationEncrypted(
            intervalSeconds: payload.intervalSeconds,
            timeZone: payload.timeZone,
            prompt: payload.prompt,
            attestationConfig: AppConfiguration.assistantAttestationVerificationConfig
        )

        if !payload.title.isEmpty {
            localTitles[created.ruleId] = payload.title
        }

        upsert(created)
        loadErrorMessage = nil
    }

    private func togglePause(for rule: AutomationRuleSummary) {
        Task { @MainActor in
            guard !mutatingRuleIDs.contains(rule.ruleId) else { return }
            mutatingRuleIDs.insert(rule.ruleId)
            defer { mutatingRuleIDs.remove(rule.ruleId) }

            do {
                let nextStatus: AutomationStatus = rule.status == .active ? .paused : .active
                let updated = try await model.apiClient.updateAutomation(
                    ruleID: rule.ruleId,
                    request: UpdateAutomationRequest(status: nextStatus)
                )
                upsert(updated)
                mutationErrorMessage = nil
            } catch {
                mutationErrorMessage = AppModel.errorMessage(from: error)
            }
        }
    }

    private func delete(rule: AutomationRuleSummary) {
        Task { @MainActor in
            guard !mutatingRuleIDs.contains(rule.ruleId) else { return }
            mutatingRuleIDs.insert(rule.ruleId)
            defer { mutatingRuleIDs.remove(rule.ruleId) }

            do {
                _ = try await model.apiClient.deleteAutomation(ruleID: rule.ruleId)
                rules.removeAll(where: { $0.ruleId == rule.ruleId })
                localTitles.removeValue(forKey: rule.ruleId)
                mutationErrorMessage = nil
            } catch {
                mutationErrorMessage = AppModel.errorMessage(from: error)
            }
        }
    }

    private func upsert(_ updated: AutomationRuleSummary) {
        if let index = rules.firstIndex(where: { $0.ruleId == updated.ruleId }) {
            rules[index] = updated
        } else {
            rules.append(updated)
        }
        rules.sort { $0.nextRunAt < $1.nextRunAt }
    }

    private func title(for rule: AutomationRuleSummary) -> String {
        if let local = localTitles[rule.ruleId], !local.isEmpty {
            return local
        }
        return "Automation \(rule.ruleId.uuidString.prefix(8))"
    }
}
