import dayjs from 'dayjs';
import {
  DefaultCountry,
  DefaultRootFontSize,
  DefaultThemeMode,
  DefaultVpnMode,
} from '../constants';
import {
  AppError,
  AppState,
  CodeDependency,
  ConnectProgressMsg,
  ConnectionState,
  Country,
  DaemonInfo,
  DaemonStatus,
  NodeHop,
  NodeLocation,
  ThemeMode,
  UiTheme,
  VpnMode,
} from '../types';

export type StateAction =
  | { type: 'init-done' }
  | { type: 'change-connection-state'; state: ConnectionState }
  | { type: 'set-daemon-status'; status: DaemonStatus }
  | { type: 'set-daemon-info'; info: DaemonInfo }
  | { type: 'set-vpn-mode'; mode: VpnMode }
  | { type: 'set-error'; error: AppError }
  | { type: 'reset-error' }
  | { type: 'new-progress-message'; message: ConnectProgressMsg }
  | { type: 'connect' }
  | { type: 'disconnect' }
  | { type: 'set-version'; version: string }
  | { type: 'set-connected'; startTime: number }
  | { type: 'set-connection-start-time'; startTime?: number | null }
  | { type: 'set-disconnected' }
  | { type: 'set-auto-connect'; autoConnect: boolean }
  | { type: 'set-monitoring'; monitoring: boolean }
  | { type: 'set-desktop-notifications'; enabled: boolean }
  | { type: 'reset' }
  | { type: 'set-ui-theme'; theme: UiTheme }
  | { type: 'set-theme-mode'; mode: ThemeMode }
  | { type: 'system-theme-changed'; theme: UiTheme }
  | {
      type: 'set-country-list';
      payload: { hop: NodeHop; countries: Country[] };
    }
  | {
      type: 'set-fast-country-list';
      payload: { countries: Country[] };
    }
  | {
      type: 'set-countries-loading';
      payload: { hop: NodeHop; loading: boolean };
    }
  | {
      type: 'set-node-location';
      payload: { hop: NodeHop; location: NodeLocation };
    }
  | { type: 'set-fastest-node-location'; country: Country }
  | { type: 'set-root-font-size'; size: number }
  | { type: 'set-code-deps-js'; dependencies: CodeDependency[] }
  | { type: 'set-code-deps-rust'; dependencies: CodeDependency[] }
  | { type: 'set-account'; stored: boolean }
  | { type: 'set-entry-countries-error'; payload: AppError | null }
  | { type: 'set-exit-countries-error'; payload: AppError | null };

export const initialState: AppState = {
  initialized: false,
  state: 'Disconnected',
  daemonStatus: 'NotOk',
  version: null,
  loading: false,
  vpnMode: DefaultVpnMode,
  uiTheme: 'Light',
  themeMode: DefaultThemeMode,
  progressMessages: [],
  autoConnect: false,
  monitoring: false,
  desktopNotifications: true,
  // TODO ⚠ these should be set to 'Fastest' when the backend is ready
  entryNodeLocation: DefaultCountry,
  // TODO ⚠ these should be set to 'Fastest' when the backend is ready
  exitNodeLocation: DefaultCountry,
  fastestNodeLocation: DefaultCountry,
  entryCountryList: [],
  exitCountryList: [],
  entryCountriesLoading: true,
  exitCountriesLoading: true,
  rootFontSize: DefaultRootFontSize,
  codeDepsRust: [],
  codeDepsJs: [],
  account: false,
  fetchMnCountries: async () => {
    /*  SCARECROW */
  },
  fetchWgCountries: async () => {
    /* SCARECROW */
  },
};

export function reducer(state: AppState, action: StateAction): AppState {
  switch (action.type) {
    case 'init-done':
      return {
        ...state,
        initialized: true,
      };
    case 'set-daemon-status':
      return {
        ...state,
        daemonStatus: action.status,
      };
    case 'set-daemon-info':
      return {
        ...state,
        daemonVersion: action.info.version,
        networkEnv: action.info.network,
      };
    case 'set-node-location':
      if (action.payload.hop === 'entry') {
        return {
          ...state,
          entryNodeLocation: action.payload.location,
        };
      }
      return {
        ...state,
        exitNodeLocation: action.payload.location,
      };
    case 'set-vpn-mode':
      return {
        ...state,
        vpnMode: action.mode,
      };
    case 'set-auto-connect':
      return {
        ...state,
        autoConnect: action.autoConnect,
      };
    case 'set-monitoring':
      return {
        ...state,
        monitoring: action.monitoring,
      };
    case 'set-desktop-notifications':
      return {
        ...state,
        desktopNotifications: action.enabled,
      };
    case 'set-country-list':
      if (action.payload.hop === 'entry') {
        return {
          ...state,
          entryCountryList: action.payload.countries,
        };
      }
      return {
        ...state,
        exitCountryList: action.payload.countries,
      };
    case 'set-fast-country-list':
      return {
        ...state,
        entryCountryList: action.payload.countries,
        exitCountryList: action.payload.countries,
      };
    case 'set-countries-loading':
      if (action.payload.hop === 'entry') {
        return {
          ...state,
          entryCountriesLoading: action.payload.loading,
        };
      }
      return {
        ...state,
        exitCountriesLoading: action.payload.loading,
      };
    case 'change-connection-state': {
      if (action.state === state.state) {
        return state;
      }
      return {
        ...state,
        state: action.state,
        loading:
          action.state === 'Connecting' || action.state === 'Disconnecting',
      };
    }
    case 'connect': {
      return { ...state, state: 'Connecting', loading: true };
    }
    case 'disconnect': {
      return { ...state, state: 'Disconnecting', loading: true };
    }
    case 'set-version':
      return {
        ...state,
        version: action.version,
      };
    case 'set-connected': {
      return {
        ...state,
        state: 'Connected',
        loading: false,
        progressMessages: [],
        sessionStartDate: dayjs.unix(action.startTime),
      };
    }
    case 'set-disconnected': {
      return {
        ...state,
        state: 'Disconnected',
        loading: false,
        progressMessages: [],
        sessionStartDate: null,
      };
    }
    case 'set-account':
      return { ...state, account: action.stored };
    case 'set-connection-start-time':
      return {
        ...state,
        sessionStartDate:
          (action.startTime && dayjs.unix(action.startTime)) || null,
      };
    case 'set-error':
      return { ...state, error: action.error };
    case 'reset-error':
      return { ...state, error: null };
    case 'new-progress-message':
      return {
        ...state,
        progressMessages: [...state.progressMessages, action.message],
      };
    case 'set-ui-theme':
      return {
        ...state,
        uiTheme: action.theme,
      };
    case 'set-theme-mode':
      return {
        ...state,
        themeMode: action.mode,
      };
    case 'system-theme-changed':
      if (state.themeMode === 'System' && state.uiTheme !== action.theme) {
        return {
          ...state,
          uiTheme: action.theme,
        };
      }
      return state;
    case 'set-fastest-node-location':
      return {
        ...state,
        fastestNodeLocation: action.country,
      };
    case 'set-root-font-size':
      return {
        ...state,
        rootFontSize: action.size,
      };
    case 'set-code-deps-js':
      return {
        ...state,
        codeDepsJs: action.dependencies,
      };
    case 'set-code-deps-rust':
      return {
        ...state,
        codeDepsRust: action.dependencies,
      };
    case 'set-entry-countries-error':
      return {
        ...state,
        entryCountriesError: action.payload,
      };
    case 'set-exit-countries-error':
      return {
        ...state,
        exitCountriesError: action.payload,
      };

    case 'reset':
      return initialState;
  }
}
