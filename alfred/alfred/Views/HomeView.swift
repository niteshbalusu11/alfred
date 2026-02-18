import ClerkKit
import SwiftUI

struct HomeView: View {
    @ObservedObject var model: AppModel

    var body: some View {
        VStack(spacing: 0) {
            HomeVoiceTranscriptionSection(model: model)
                .frame(maxWidth: 560, maxHeight: .infinity, alignment: .top)
                .padding(.horizontal, AppTheme.Layout.screenPadding)
                .padding(.top, 12)
                .padding(.bottom, 10)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .appScreenBackground()
    }
}

#Preview {
    let clerk = Clerk.preview()
    HomeView(model: AppModel(clerk: clerk))
}
