import ClerkKit
import SwiftUI

struct ProfileView: View {
    @Environment(Clerk.self) private var clerk
    @ObservedObject var model: AppModel
    @State private var showSignOutConfirmation = false

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Profile moved")
                .font(.title2.weight(.bold))
                .foregroundStyle(AppTheme.Colors.textPrimary)

            Text("Use the top-right account button for profile and account controls.")
                .font(.subheadline)
                .foregroundStyle(AppTheme.Colors.textSecondary)

            if let accountDisplayName {
                Text(accountDisplayName)
                    .font(.headline)
                    .foregroundStyle(AppTheme.Colors.textPrimary)
            }

            if let accountEmail {
                Text(accountEmail)
                    .font(.footnote)
                    .foregroundStyle(AppTheme.Colors.textSecondary)
            }

            Button("Sign out") {
                showSignOutConfirmation = true
            }
            .buttonStyle(.appSecondary)

            Spacer()
        }
        .padding(.horizontal, AppTheme.Layout.screenPadding)
        .padding(.vertical, AppTheme.Layout.sectionSpacing)
        .appScreenBackground()
        .confirmationDialog("Sign out of Alfred?", isPresented: $showSignOutConfirmation) {
            Button("Sign out", role: .destructive) {
                Task {
                    await model.signOut()
                }
            }
            Button("Cancel", role: .cancel) {}
        }
    }

    private var accountDisplayName: String? {
        let firstName = clerk.user?.firstName?.trimmingCharacters(in: .whitespacesAndNewlines)
        let lastName = clerk.user?.lastName?.trimmingCharacters(in: .whitespacesAndNewlines)
        let username = clerk.user?.username?.trimmingCharacters(in: .whitespacesAndNewlines)

        if let firstName, let lastName, !firstName.isEmpty, !lastName.isEmpty {
            return "\(firstName) \(lastName)"
        }
        if let firstName, !firstName.isEmpty {
            return firstName
        }
        if let lastName, !lastName.isEmpty {
            return lastName
        }
        if let username, !username.isEmpty {
            return username
        }
        return nil
    }

    private var accountEmail: String? {
        clerk.user?.primaryEmailAddress?.emailAddress
    }
}

#Preview {
    let clerk = Clerk.preview()
    ProfileView(model: AppModel(clerk: clerk))
        .environment(clerk)
}
