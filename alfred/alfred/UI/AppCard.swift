import SwiftUI

struct AppCard<Content: View>: View {
    let content: Content

    init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    var body: some View {
        content
            .padding(AppTheme.Layout.cardPadding)
            .background(AppTheme.Colors.surface)
            .clipShape(RoundedRectangle(cornerRadius: AppTheme.Layout.cardCornerRadius, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: AppTheme.Layout.cardCornerRadius, style: .continuous)
                    .stroke(AppTheme.Colors.outline, lineWidth: AppTheme.Layout.cartoonStrokeWidth)
            )
            .shadow(
                color: AppTheme.Colors.shadow.opacity(0.36),
                radius: AppTheme.Layout.softShadowRadius,
                x: 0,
                y: AppTheme.Layout.cartoonShadowOffset
            )
    }
}

struct AppSectionHeader<Accessory: View>: View {
    let title: String
    let subtitle: String?
    let accessory: Accessory

    init(_ title: String, subtitle: String? = nil, @ViewBuilder accessory: () -> Accessory) {
        self.title = title
        self.subtitle = subtitle
        self.accessory = accessory()
    }

    init(_ title: String, subtitle: String? = nil) where Accessory == EmptyView {
        self.title = title
        self.subtitle = subtitle
        self.accessory = EmptyView()
    }

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 12) {
            VStack(alignment: .leading, spacing: 4) {
                Text(title)
                    .font(.headline)
                    .foregroundStyle(AppTheme.Colors.textPrimary)

                if let subtitle {
                    Text(subtitle)
                        .font(.footnote)
                        .foregroundStyle(AppTheme.Colors.textSecondary)
                }
            }

            Spacer(minLength: 12)
            accessory
        }
    }
}
