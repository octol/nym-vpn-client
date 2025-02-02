package net.nymtech.nymvpn.util

object Constants {

	const val APP_PROJECT_NAME = "nym-vpn-android"

	const val LANGUAGE_SWITCH_DELAY = 2000L
	const val VPN_API_BASE_URL = "https://nymvpn.com/api/"
	const val SUBSCRIPTION_TIMEOUT = 5_000L
	const val LOG_BUFFER_SIZE = 5_000L

	const val EMAIL_MIME_TYPE = "message/rfc822"
	const val TEXT_MIME_TYPE = "text/plain"
	const val BASE_LOG_FILE_NAME = "nym_vpn_logs"

	const val SENTRY_DEV_ENV = "development"
	const val SENTRY_PROD_ENV = "production"

	const val NYM_VPN_LIB_TAG = "libnymvpn"

	// testing stuff
	const val CONNECT_TEST_TAG = "connectTag"
	const val LOGIN_TEST_TAG = "loginTag"
	const val DISCONNECT_TEST_TAG = "disconnectTag"

	const val VPN_SETTINGS_PACKAGE = "android.net.vpn.SETTINGS"

	const val FREE_PASS_CRED_DURATION = 30f

	const val KOTLIN_LICENSES_ASSET_FILE_NAME = "artifacts.json"
	const val RUST_LICENSES_ASSET_FILE_NAME = "licenses_rust.json"

	const val PROVIDER = ""
}
