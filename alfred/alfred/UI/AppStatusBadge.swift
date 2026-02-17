import SwiftUI

struct AppStatusBadge: View {
    enum Style {
        case neutral
        case success
        case warning
        case danger

        var background: Color {
            switch self {
            case .neutral:
                return AppTheme.Colors.surfaceElevated
            case .success:
                return AppTheme.Colors.success.opacity(0.2)
            case .warning:
                return AppTheme.Colors.warning.opacity(0.2)
            case .danger:
                return AppTheme.Colors.danger.opacity(0.2)
            }
        }

        var foreground: Color {
            switch self {
            case .neutral:
                return AppTheme.Colors.textSecondary
            case .success:
                return AppTheme.Colors.success
            case .warning:
                return AppTheme.Colors.warning
            case .danger:
                return AppTheme.Colors.danger
            }
        }
    }

    let title: String
    let style: Style

    var body: some View {
        Text(title)
            .font(.caption.weight(.semibold))
            .foregroundStyle(style.foreground)
            .padding(.horizontal, 10)
            .padding(.vertical, 4)
            .background(style.background)
            .clipShape(Capsule())
            .overlay(
                Capsule()
                    .stroke(AppTheme.Colors.outline, lineWidth: 1)
            )
    }
}
