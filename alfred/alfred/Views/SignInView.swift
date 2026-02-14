import SwiftUI

struct SignInView: View {
    @ObservedObject var model: AppModel

    var body: some View {
        Form {
            Section("API") {
                Text(model.apiBaseURL.absoluteString)
                    .font(.footnote)
                    .textSelection(.enabled)
            }

            Section("Sign In") {
                SecureField("Apple identity token", text: $model.appleIdentityToken)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()

                TextField("Device ID", text: $model.deviceID)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()

                Button("Create Session") {
                    Task {
                        await model.signIn()
                    }
                }
                .disabled(model.isLoading(.signIn))

                if model.isLoading(.signIn) {
                    ProgressView()
                }
            }
        }
    }
}
