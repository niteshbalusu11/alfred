import AlfredAPIClient
import SwiftUI

struct AutomationsView: View {
    @ObservedObject var model: AppModel
    @State private var rules: [AutomationRuleSummary] = []
    @State private var cachedPrompts: [UUID: String] = [:]
    @State private var isLoading = false
    @State private var loadErrorMessage: String?
    @State private var mutationErrorMessage: String?
    @State private var mutatingRuleIDs: Set<UUID> = []
    @State private var activeSheet: SheetRoute?
    private let automationRuleStore = AutomationRuleStore()

    private enum SheetRoute: Hashable, Identifiable {
        case create
        case edit(UUID)

        var id: String {
            switch self {
            case .create:
                return "create"
            case .edit(let ruleID):
                return "edit-\(ruleID.uuidString.lowercased())"
            }
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            header

            if isLoading && rules.isEmpty {
                ScrollView {
                    AutomationLoadingStateCard()
                        .padding(.horizontal, AppTheme.Layout.screenPadding)
                        .padding(.vertical, AppTheme.Layout.sectionSpacing)
                }
            } else if rules.isEmpty, loadErrorMessage == nil {
                TaskEmptyStateHero {
                    activeSheet = .create
                }
            } else {
                ScrollView {
                    LazyVStack(spacing: AppTheme.Layout.sectionSpacing) {
                        if let mutationErrorMessage {
                            AutomationCallout(
                                title: "Task action failed",
                                message: mutationErrorMessage,
                                buttonTitle: "Dismiss"
                            ) {
                                self.mutationErrorMessage = nil
                            }
                        }

                        if let loadErrorMessage, rules.isEmpty {
                            AutomationEmptyStateCard(
                                title: "Unable to load tasks",
                                message: loadErrorMessage,
                                buttonTitle: "Retry"
                            ) {
                                Task {
                                    await loadAutomations()
                                }
                            }
                        } else {
                            ForEach(rules, id: \.ruleId) { rule in
                                AutomationRuleCard(
                                    title: rule.title,
                                    rule: rule,
                                    isMutating: mutatingRuleIDs.contains(rule.ruleId),
                                    onEdit: { activeSheet = .edit(rule.ruleId) },
                                    onTogglePause: { togglePause(for: rule) },
                                    onDelete: { delete(rule: rule) },
                                    onDebugRun: debugRunAction(for: rule)
                                )
                            }

                            if let loadErrorMessage {
                                AutomationCallout(
                                    title: "Could not refresh tasks",
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
                    .padding(.horizontal, AppTheme.Layout.screenPadding)
                    .padding(.bottom, AppTheme.Layout.sectionSpacing)
                    .padding(.top, 8)
                }
            }
        }
        .appScreenBackground()
        .task {
            if rules.isEmpty {
                await loadCachedAutomations()
                await loadAutomations()
            }
        }
        .fullScreenCover(item: $activeSheet) { route in
            switch route {
            case .create:
                AutomationEditorSheet(
                    mode: .create,
                    defaultTimeZone: TimeZone.current.identifier,
                    onSubmit: { payload in
                        try await createAutomation(payload)
                    }
                )
            case .edit(let ruleID):
                if let rule = rules.first(where: { $0.ruleId == ruleID }) {
                    AutomationEditorSheet(
                        mode: .edit(existing: rule, existingPrompt: cachedPrompts[rule.ruleId]),
                        defaultTimeZone: rule.schedule.timeZone,
                        onSubmit: { payload in
                            try await updateAutomation(ruleID: ruleID, payload: payload)
                        }
                    )
                } else {
                    Color.clear
                        .onAppear {
                            activeSheet = nil
                        }
                }
            }
        }
    }

    private var header: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 3) {
                Text("Tasks")
                    .font(.title2.weight(.semibold))
                    .foregroundStyle(AppTheme.Colors.textPrimary)

                Text("Scheduled prompts with private push delivery")
                    .font(.footnote)
                    .foregroundStyle(AppTheme.Colors.textSecondary)
            }

            Spacer(minLength: 0)

            TaskHeaderIconButton(
                systemImage: isLoading ? "arrow.triangle.2.circlepath.circle.fill" : "arrow.clockwise",
                accessibilityLabel: "Refresh tasks",
                action: {
                    Task {
                        await loadAutomations()
                    }
                }
            )
            .disabled(isLoading)
            .opacity(isLoading ? 0.55 : 1)

            TaskHeaderIconButton(
                systemImage: "plus",
                accessibilityLabel: "Create task",
                action: {
                    activeSheet = .create
                }
            )
        }
        .padding(.horizontal, AppTheme.Layout.screenPadding)
        .padding(.top, 8)
        .padding(.bottom, 10)
    }

    @MainActor
    private func loadAutomations() async {
        guard !isLoading else { return }
        isLoading = true
        defer { isLoading = false }

        do {
            let response = try await model.apiClient.listAutomations(limit: 100)
            let fetchedRules = response.items.sorted(by: { $0.nextRunAt < $1.nextRunAt })
            cachedPrompts = mergedPromptCache(for: fetchedRules)
            rules = fetchedRules
            await persistCachedAutomations()
            loadErrorMessage = nil
        } catch {
            loadErrorMessage = AppModel.errorMessage(from: error)
        }
    }

    @MainActor
    private func createAutomation(_ payload: AutomationEditorPayload) async throws {
        guard let prompt = payload.prompt else {
            throw AlfredAPIClientError.serverError(
                statusCode: 400,
                code: "invalid_prompt",
                message: "Prompt is required"
            )
        }

        let created = try await model.apiClient.createAutomationEncrypted(
            title: payload.title,
            schedule: payload.schedule,
            prompt: prompt,
            attestationConfig: AppConfiguration.assistantAttestationVerificationConfig
        )

        upsert(created, prompt: payload.prompt)
        await persistCachedAutomations()
        loadErrorMessage = nil
        mutationErrorMessage = nil
    }

    @MainActor
    private func updateAutomation(ruleID: UUID, payload: AutomationEditorPayload) async throws {
        let updated = try await model.apiClient.updateAutomationEncrypted(
            ruleID: ruleID,
            title: payload.title,
            schedule: payload.schedule,
            prompt: payload.prompt,
            status: nil,
            attestationConfig: AppConfiguration.assistantAttestationVerificationConfig
        )
        upsert(updated, prompt: payload.prompt)
        await persistCachedAutomations()
        mutationErrorMessage = nil
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
                await persistCachedAutomations()
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
                cachedPrompts.removeValue(forKey: rule.ruleId)
                await persistCachedAutomations()
                mutationErrorMessage = nil
            } catch {
                mutationErrorMessage = AppModel.errorMessage(from: error)
            }
        }
    }

    private func debugRun(rule: AutomationRuleSummary) {
        Task { @MainActor in
            guard !mutatingRuleIDs.contains(rule.ruleId) else { return }
            mutatingRuleIDs.insert(rule.ruleId)
            defer { mutatingRuleIDs.remove(rule.ruleId) }

            do {
                _ = try await model.apiClient.triggerAutomationDebugRun(ruleID: rule.ruleId)
                mutationErrorMessage = nil
            } catch {
                mutationErrorMessage = AppModel.errorMessage(from: error)
            }
        }
    }

    private func debugRunAction(for rule: AutomationRuleSummary) -> (() -> Void)? {
        #if DEBUG
            return { debugRun(rule: rule) }
        #else
            return nil
        #endif
    }

    private func upsert(_ updated: AutomationRuleSummary, prompt: String? = nil) {
        let previous = rules.first(where: { $0.ruleId == updated.ruleId })
        if let index = rules.firstIndex(where: { $0.ruleId == updated.ruleId }) {
            rules[index] = updated
        } else {
            rules.append(updated)
        }
        rules.sort { $0.nextRunAt < $1.nextRunAt }

        if let prompt {
            let trimmed = prompt.trimmingCharacters(in: .whitespacesAndNewlines)
            if trimmed.isEmpty {
                cachedPrompts.removeValue(forKey: updated.ruleId)
            } else {
                cachedPrompts[updated.ruleId] = trimmed
            }
        } else if let previous, previous.promptSha256 != updated.promptSha256 {
            cachedPrompts.removeValue(forKey: updated.ruleId)
        }
    }

    @MainActor
    private func loadCachedAutomations() async {
        guard let userID = automationStorageUserID else { return }

        do {
            let snapshot = try await automationRuleStore.load(for: userID)
            rules = snapshot.entries.map(\.rule).sorted(by: { $0.nextRunAt < $1.nextRunAt })
            cachedPrompts = snapshot.promptByRuleID
        } catch {
            rules = []
            cachedPrompts = [:]
        }
    }

    @MainActor
    private func persistCachedAutomations() async {
        guard let userID = automationStorageUserID else { return }
        let entries = rules.map { rule in
            AutomationRuleCacheEntry(
                rule: rule,
                prompt: cachedPrompts[rule.ruleId]
            )
        }

        do {
            try await automationRuleStore.save(AutomationRuleCacheSnapshot(entries: entries), for: userID)
        } catch {
            // Best-effort cache only; server remains source of truth.
        }
    }

    private func mergedPromptCache(for fetchedRules: [AutomationRuleSummary]) -> [UUID: String] {
        let previousByRuleID = Dictionary(uniqueKeysWithValues: rules.map { ($0.ruleId, $0) })
        var merged: [UUID: String] = [:]

        for rule in fetchedRules {
            guard let cachedPrompt = cachedPrompts[rule.ruleId] else { continue }
            guard let previousRule = previousByRuleID[rule.ruleId] else { continue }
            guard previousRule.promptSha256 == rule.promptSha256 else { continue }
            merged[rule.ruleId] = cachedPrompt
        }

        return merged
    }

    private var automationStorageUserID: String? {
        guard let value = model.assistantStorageUserID?.trimmingCharacters(in: .whitespacesAndNewlines),
              !value.isEmpty
        else {
            return nil
        }
        return value
    }
}
