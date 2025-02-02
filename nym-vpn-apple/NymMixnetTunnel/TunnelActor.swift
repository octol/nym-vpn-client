import Logging
import NetworkExtension
import UserNotifications
import MixnetLibrary
import NymLogger
import Tunnels

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
    @Published private var didSendLastError = false

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

        defaultPathObservation = tunnelProvider?.observe(\.defaultPath) { [weak self] tunnelProvider, _ in
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

    func didSendLastError() async throws {
        var stateStream = $didSendLastError.values.makeAsyncIterator()
        while case let .some(newState) = await stateStream.next() {
            switch newState {
            case true:
                throw PacketTunnelProviderError.backendStartFailure
            case false:
                break
            }
        }
    }

    func setDidSendLastError(with value: Bool) {
        didSendLastError = value
    }
}

private extension TunnelActor {
    func scheduleDisconnectNotification() async {
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
