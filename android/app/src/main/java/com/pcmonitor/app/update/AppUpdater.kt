package com.pcmonitor.app.update

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageInstaller
import android.net.Uri
import android.os.Build
import android.provider.Settings
import com.pcmonitor.app.data.remote.ConvexHttp
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONObject
import java.io.File
import java.security.MessageDigest
import java.util.concurrent.TimeUnit

/** A newer app build published by the release workflow (convex releases:latest). */
data class AppRelease(val version: String, val versionCode: Long, val apkUrl: String, val sha256: String)

sealed interface UpdateState {
    data object Idle : UpdateState
    data class Available(val release: AppRelease, val installedVersion: String) : UpdateState
    data class Downloading(val release: AppRelease, val progress: Float) : UpdateState
    data class Installing(val release: AppRelease) : UpdateState
    data class Failed(val release: AppRelease, val message: String) : UpdateState
}

/**
 * In-app updates for the sideloaded APK. The app is not on Play, so Android asks the user to
 * confirm each install (on Android 12+ later updates can go through without asking once Vitals
 * itself installed the previous one). Android also refuses any APK not signed with the same key
 * as the installed app, on top of the SHA-256 check here.
 */
object AppUpdater {
    private const val APK_URL_PREFIX = "https://github.com/theallmyti/Vitals/releases/download/"
    private const val MAX_APK_BYTES = 200L * 1024 * 1024

    private val http = OkHttpClient.Builder()
        .connectTimeout(15, TimeUnit.SECONDS)
        .readTimeout(60, TimeUnit.SECONDS)
        .build()

    /** Outlives the dialog, so rotating the phone or a theme change does not cancel a download. */
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)

    private val _state = MutableStateFlow<UpdateState>(UpdateState.Idle)
    val state: StateFlow<UpdateState> = _state.asStateFlow()

    /** "Later" hides the dialog until the app process restarts. */
    private var dismissedThisSession = false

    fun dismiss() {
        dismissedThisSession = true
        _state.value = UpdateState.Idle
    }

    /** Asks the backend for the published app; shows the dialog if it is newer than this build. */
    suspend fun check(context: Context) {
        if (dismissedThisSession || _state.value !is UpdateState.Idle) return
        val release = fetchLatest() ?: return
        val installed = context.packageManager.getPackageInfo(context.packageName, 0)
        val installedCode = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) installed.longVersionCode
            else @Suppress("DEPRECATION") installed.versionCode.toLong()
        if (release.versionCode > installedCode) {
            _state.value = UpdateState.Available(release, installed.versionName ?: "?")
        }
    }

    private suspend fun fetchLatest(): AppRelease? = withContext(Dispatchers.IO) {
        try {
            val req = ConvexHttp.request("query", "releases:latest", JSONObject())
            val env = http.newCall(req).execute().use { JSONObject(it.body?.string().orEmpty()) }
            val v = env.optJSONObject("value")
            if (env.optString("status") != "success" || v == null || v.isNull("apkUrl")) return@withContext null
            val release = AppRelease(
                version = v.getString("appVersion"),
                versionCode = v.getLong("appVersionCode"),
                apkUrl = v.getString("apkUrl"),
                sha256 = v.getString("apkSha256").lowercase(),
            )
            // Never download from anywhere but this repo's releases, whatever the backend says.
            if (!release.apkUrl.startsWith(APK_URL_PREFIX) || !Regex("^[0-9a-f]{64}$").matches(release.sha256)) {
                return@withContext null
            }
            release
        } catch (e: Exception) {
            null // offline or an older backend without releases:latest: try again next launch
        }
    }

    /**
     * Downloads, verifies and hands the APK to Android's installer. Returns false (and opens the
     * settings page) when Vitals is not yet allowed to install apps; the user taps Update again.
     */
    fun startUpdate(context: Context, release: AppRelease): Boolean {
        val app = context.applicationContext
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O && !app.packageManager.canRequestPackageInstalls()) {
            context.startActivity(
                Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${app.packageName}"))
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            )
            return false
        }
        if (_state.value is UpdateState.Downloading || _state.value is UpdateState.Installing) return true
        _state.value = UpdateState.Downloading(release, 0f)
        scope.launch {
            try {
                val apk = download(app, release)
                _state.value = UpdateState.Installing(release)
                install(app, apk)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                _state.value = UpdateState.Failed(release, e.message ?: "Download failed")
            }
        }
        return true
    }

    private suspend fun download(context: Context, release: AppRelease): File = withContext(Dispatchers.IO) {
        val dir = File(context.cacheDir, "updates").apply { mkdirs() }
        dir.listFiles()?.forEach { it.delete() } // old downloads
        val out = File(dir, "vitals-${release.version}.apk")
        val digest = MessageDigest.getInstance("SHA-256")
        http.newCall(Request.Builder().url(release.apkUrl).build()).execute().use { resp ->
            if (!resp.isSuccessful) error("Download failed (HTTP ${resp.code})")
            val body = resp.body ?: error("Empty download")
            val total = body.contentLength()
            if (total > MAX_APK_BYTES) error("Download is too large")
            var read = 0L
            body.byteStream().use { input ->
                out.outputStream().use { output ->
                    val buf = ByteArray(64 * 1024)
                    while (true) {
                        val n = input.read(buf)
                        if (n < 0) break
                        read += n
                        if (read > MAX_APK_BYTES) error("Download is too large")
                        digest.update(buf, 0, n)
                        output.write(buf, 0, n)
                        if (total > 0) _state.value = UpdateState.Downloading(release, read.toFloat() / total)
                    }
                }
            }
        }
        val hash = digest.digest().joinToString("") { "%02x".format(it) }
        if (hash != release.sha256) {
            out.delete()
            error("The download was damaged or tampered with. Try again.")
        }
        out
    }

    private suspend fun install(context: Context, apk: File) = withContext(Dispatchers.IO) {
        val installer = context.packageManager.packageInstaller
        val params = PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL).apply {
            setAppPackageName(context.packageName)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                setRequireUserAction(PackageInstaller.SessionParams.USER_ACTION_NOT_REQUIRED)
            }
        }
        val sessionId = installer.createSession(params)
        installer.openSession(sessionId).use { session ->
            session.openWrite("vitals.apk", 0, apk.length()).use { out ->
                apk.inputStream().use { it.copyTo(out) }
                session.fsync(out)
            }
            val flags = PendingIntent.FLAG_UPDATE_CURRENT or
                (if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) PendingIntent.FLAG_MUTABLE else 0)
            val callback = PendingIntent.getBroadcast(
                context, sessionId, Intent(context, InstallResultReceiver::class.java), flags
            )
            session.commit(callback.intentSender)
        }
    }

    /** Called by InstallResultReceiver when Android reports the install failed or was cancelled. */
    internal fun onInstallFailed(message: String) {
        val current = _state.value
        val release = when (current) {
            is UpdateState.Installing -> current.release
            is UpdateState.Downloading -> current.release
            is UpdateState.Available -> current.release
            is UpdateState.Failed -> current.release
            UpdateState.Idle -> return
        }
        _state.value = UpdateState.Failed(release, message)
    }
}
