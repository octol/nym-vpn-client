import Foundation

public enum PacketTunnelProviderError: String, Error {
    case invalidSavedConfiguration
    case noCredentialDataDir
    case startAccountController
    case backendStartFailure
}
