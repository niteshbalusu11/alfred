import SwiftUI

struct TaskHeaderIconButton: View {
    let systemImage: String
    let accessibilityLabel: String
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: systemImage)
                .font(.system(size: 16, weight: .semibold))
                .foregroundStyle(AppTheme.Colors.textPrimary)
                .frame(width: 38, height: 38)
                .background(AppTheme.Colors.surfaceElevated, in: Circle())
                .overlay(
                    Circle()
                        .stroke(AppTheme.Colors.outline, lineWidth: 1)
                )
        }
        .buttonStyle(.plain)
        .accessibilityLabel(accessibilityLabel)
    }
}

struct TaskEmptyStateHero: View {
    let onCreate: () -> Void

    var body: some View {
        VStack(spacing: 14) {
            Spacer(minLength: 0)

            ZStack {
                Circle()
                    .fill(AppTheme.Colors.surfaceElevated)
                    .frame(width: 78, height: 78)

                Image(systemName: "alarm")
                    .font(.system(size: 30, weight: .medium))
                    .foregroundStyle(AppTheme.Colors.textPrimary.opacity(0.9))
            }

            Text("Get started by adding a task")
                .font(.title3.weight(.semibold))
                .foregroundStyle(AppTheme.Colors.textPrimary)

            Text("Schedule a task to automate any prompt and get reminded when it completes")
                .font(.subheadline)
                .foregroundStyle(AppTheme.Colors.textSecondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 28)

            Button(action: onCreate) {
                HStack(spacing: 8) {
                    Image(systemName: "plus")
                    Text("Create Task")
                }
                .font(.headline.weight(.semibold))
                .foregroundStyle(AppTheme.Colors.textPrimary)
                .padding(.horizontal, 20)
                .padding(.vertical, 11)
                .background(AppTheme.Colors.surface.opacity(0.96), in: Capsule())
                .overlay(
                    Capsule()
                        .stroke(AppTheme.Colors.outline, lineWidth: 1)
                )
            }
            .buttonStyle(.plain)
            .padding(.top, 4)

            Spacer(minLength: 0)
        }
        .padding(.horizontal, AppTheme.Layout.screenPadding)
        .padding(.bottom, 60)
    }
}

struct TaskPickerRow<Trailing: View>: View {
    let label: String
    let trailing: Trailing

    init(label: String, @ViewBuilder trailing: () -> Trailing) {
        self.label = label
        self.trailing = trailing()
    }

    var body: some View {
        HStack(spacing: 12) {
            Text(label)
                .font(.headline.weight(.medium))
                .foregroundStyle(AppTheme.Colors.textPrimary)

            Spacer(minLength: 0)
            trailing
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 12)
    }
}
