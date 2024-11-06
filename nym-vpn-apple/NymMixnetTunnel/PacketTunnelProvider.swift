import NetworkExtension
import UserNotifications
import Logging
import ConfigurationManager
import NymLogger
import MixnetLibrary
import TunnelMixnet
import Tunnels

class PacketTunnelProvider: NEPacketTunnelProvider {
    lazy var logger = Logger(label: "MixnetTunnel")

    let tunnelActor: TunnelActor

    override init() {
        LoggingSystem.bootstrap { label in
            let fileLogHandler = FileLogHandler(label: label, logFileManager: LogFileManager(logFileType: .tunnel))

            #if DEBUG
                let osLogHandler = OSLogHandler(
                    subsystem: Bundle.main.bundleIdentifier ?? "NymMixnetTunnel",
                    category: label
                )
                return MultiplexLogHandler([osLogHandler, fileLogHandler])
            #else
                return fileLogHandler
            #endif
        }

        tunnelActor = TunnelActor()
    }

    override func startTunnel(options: [String: NSObject]? = nil) async throws {
        logger.info("Start tunnel...")

        initLogger()

        setup()

        await tunnelActor.setTunnelProvider(self)

        guard let tunnelProviderProtocol = protocolConfiguration as? NETunnelProviderProtocol,
              let mixnetConfig = tunnelProviderProtocol.asMixnetConfig()
        else {
            logger.error("Failed to obtain tunnel configuration")
            throw PacketTunnelProviderError.invalidSavedConfiguration
        }

        let vpnConfig = try mixnetConfig.asVpnConfig(tunProvider: self, tunStatusListener: self)

        logger.info("Starting backend")

        guard let credentialDataPath = vpnConfig.credentialDataPath else {
            throw PacketTunnelProviderError.noCredentialDataDir
        }

        do {
            try startAccountController(dataDir: credentialDataPath)
        } catch {
            throw PacketTunnelProviderError.startAccountController
        }

        do {
            try startVpn(config: vpnConfig)
        } catch {
            logger.error("Failed to start vpn: \(error)")
            throw PacketTunnelProviderError.backendStartFailure
        }

        logger.info("Backend is up and running...")

        await tunnelActor.waitUntilStarted()
    }

    override func stopTunnel(with reason: NEProviderStopReason) async {
        logger.info("Stop tunnel... \(reason.rawValue)")

        do {
            try stopVpn()
        } catch {
            logger.error("Failed to stop the tunnel: \(error)")
        }

        do {
            try stopAccountController()
        } catch {
            logger.error("Failed to stop account controller: \(error)")
        }

        await tunnelActor.setTunnelProvider(nil)
    }

    override func handleAppMessage(_ messageData: Data) async -> Data? {
        guard let message = try? TunnelProviderMessage(messageData: messageData) else { return nil }

        switch message {
        case .lastErrorReason:
            if case let .error(reason) = await tunnelActor.tunnelState {
                do {
                    return try ErrorReason(with: reason).encode()
                } catch {
                    logger.error("Failed to encode error reason: \(error)")
                    return nil
                }
            }
        }

        return nil
    }
}

extension PacketTunnelProvider {
    func setup() {
        do {
            try ConfigurationManager.shared.setup()
        } catch {
            self.logger.error("Failed to set environment: \(error)")
        }
    }
}

extension PacketTunnelProvider: OsTunProvider {
    func setDefaultPathObserver(observer: (any OsDefaultPathObserver)?) async throws {
        await tunnelActor.setDefaultPathObserver(observer)
    }

    func setTunnelNetworkSettings(tunnelSettings: TunnelNetworkSettings) async throws {
        do {
            let networkSettings = tunnelSettings.asPacketTunnelNetworkSettings()
            logger.debug("Set network settings: \(networkSettings)")
            try await setTunnelNetworkSettings(networkSettings)
        } catch {
            logger.error("Failed to set tunnel network settings: \(error)")
            throw error
        }
    }
}

extension PacketTunnelProvider: TunnelStatusListener {
    func onEvent(event: MixnetLibrary.TunnelEvent) {
        tunnelActor.onEvent(event)
    }
}

actor TunnelActor {
    private let eventContinuation: AsyncStream<TunnelEvent>.Continuation
    private let defaultPathContinuation: AsyncStream<NWPath>.Continuation

    private let logger = Logger(label: "TunnelActor")

    weak var tunnelProvider: NEPacketTunnelProvider?

    var defaultPathObserver: (any OsDefaultPathObserver)?
    var defaultPathObservation: NSKeyValueObservation?

    /// Flag used to determine if `reasserting` property of tunnel provider can be used.
    /// Note that we shouldn't reassert unless we returned from `startTunnel()`
    var canReassert = false

    @Published private(set) var tunnelState: TunnelState?

    init() {
        let (eventStream, eventContinuation) = AsyncStream<TunnelEvent>.makeStream()
        self.eventContinuation = eventContinuation

        let (defaultPathStream, defaultPathContinuation) = AsyncStream<NWPath>.makeStream()
        self.defaultPathContinuation = defaultPathContinuation

        Task.detached { [weak self, eventStream] in
            for await case let .newState(tunnelState) in eventStream {
                await self?.setCurrentState(tunnelState)
            }
        }

        Task.detached { [weak self, defaultPathStream] in
            for await newPath in defaultPathStream {
                await self?.defaultPathObserver?.onDefaultPathChange(newPath: newPath.asOsDefaultPath())
            }
        }
    }

    deinit {
        eventContinuation.finish()
        defaultPathContinuation.finish()
    }

    nonisolated func onEvent(_ event: TunnelEvent) {
        eventContinuation.yield(event)
    }

    nonisolated func onDefaultPathChange(_ newPath: NWPath) {
        defaultPathContinuation.yield(newPath)
    }

    func setTunnelProvider(_ tunnelProvider: NEPacketTunnelProvider?) {
        self.tunnelProvider = tunnelProvider

        defaultPathObservation = tunnelProvider?.observe(\.defaultPath) { [weak self] tunnelProvider, change in
            if let newPath = tunnelProvider.defaultPath {
                self?.onDefaultPathChange(newPath)
            }
        }
    }

    func setDefaultPathObserver(_ newObserver: (any OsDefaultPathObserver)?) {
        defaultPathObserver = newObserver
    }

    private func setCurrentState(_ state: TunnelState) async {
        switch state {
        case .connecting:
            if canReassert {
                tunnelProvider?.reasserting = true
            }

        case .connected:
            if canReassert {
                tunnelProvider?.reasserting = false
            }
            canReassert = true

        case .disconnecting(.error):
            await scheduleDisconnectNotification()

        default:
            break
        }

        tunnelState = state
    }

    /// Wait until the tunnel state shifted into either connected, disconnected or error state.
    func waitUntilStarted() async {
        var stateStream = $tunnelState.values.makeAsyncIterator()

        while case let .some(newState) = await stateStream.next() {
            switch newState {
            case .connected, .disconnected, .error:
                return
            case .disconnecting, .none, .connecting:
                break
            }
        }
    }

    private func scheduleDisconnectNotification() async {
        // TODO: localize the notification content.
        // TODO: move localizations to separate package
        let content = UNMutableNotificationContent()
        content.title = "The NymVPN connection has failed."
        content.body = "Please try reconnecting."
        content.sound = UNNotificationSound.default

        let request = UNNotificationRequest(identifier: "disconnectNotification", content: content, trigger: nil)

        do {
            try await UNUserNotificationCenter.current().add(request)
        } catch {
            logger.info("🚀 Notification scheduled successfully")
        }
    }
}

extension TunnelState: @retroactive CustomStringConvertible {
    public var description: String {
        switch self {
        case .disconnected:
            "Disconnected"
        case let .disconnecting(afterDisconnect):
            switch afterDisconnect {
            case .nothing:
                "Disconnecting"
            case .reconnect:
                "Disconnecting to reconnect"
            case .error:
                "Disconnecting because of an error"
            }
        case let .error(reason):
            "Error state: \(reason)"
        case let .connecting(connectionData):
            if let connectionData {
                "Connecting to \(connectionData)"
            } else {
                "Connecting..."
            }
        case let .connected(connectionData):
            "Connected to \(connectionData)"
        }
    }
}

extension ErrorStateReason: @retroactive CustomStringConvertible {
    public var description: String {
        switch self {
        case .dns, .firewall, .routing:
            "System configuration"
        case .internal:
            "Internal error"
        case .tunDevice:
            "Failure to configure tun device"
        case .tunnelProvider:
            "Tunnel provider error"
        case .invalidEntryGatewayCountry:
            "Invalid entry gateway country"
        case .invalidExitGatewayCountry:
            "Invalid exit gateway country"
        case .sameEntryAndExitGateway:
            "Same entry and exit gateway aren't supported"
        }
    }
}

extension ConnectionData: @retroactive CustomStringConvertible {
    public var description: String {
        "entry gateway: \(entryGateway), exit gateway: \(exitGateway), \(tunnel)"
    }
}

extension TunnelConnectionData: @retroactive CustomStringConvertible {
    public var description: String {
        switch self {
        case let .mixnet(data):
            "mixnet tunnel nym-address: \(data.nymAddress), exit-ipr: \(data.exitIpr), ipv4: \(data.ipv4), ipv6: \(data.ipv6)"
        case let .wireguard(data):
            "wireguard tunnel entry: \(data.entry.endpoint), exit: \(data.exit.endpoint)"
        }
    }
}
