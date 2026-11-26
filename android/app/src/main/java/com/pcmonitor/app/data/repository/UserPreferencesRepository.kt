package com.pcmonitor.app.data.repository

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.*
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.withContext
import com.pcmonitor.app.data.remote.ConvexHttp
import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONObject
import java.util.concurrent.TimeUnit

private val Context.dataStore: DataStore<Preferences> by preferencesDataStore(name = "pc_monitor_prefs")

class UserPreferencesRepository(private val context: Context) {

    private val httpClient = OkHttpClient.Builder()
        .dns(ConvexHttp.fallbackDns)
        .connectTimeout(10, TimeUnit.SECONDS)
        .readTimeout(10, TimeUnit.SECONDS)
        .build()

    private val encryptedPrefs by lazy {
        try {
            val keyGenParameterSpec = androidx.security.crypto.MasterKeys.AES256_GCM_SPEC
            val masterKeyAlias = androidx.security.crypto.MasterKeys.getOrCreate(keyGenParameterSpec)
            val open = { name: String ->
                androidx.security.crypto.EncryptedSharedPreferences.create(
                    name,
                    masterKeyAlias,
                    context,
                    androidx.security.crypto.EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
                    androidx.security.crypto.EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM
                )
            }
            open("vitals_encrypted_session").also { it.adoptLegacySession(LEGACY_ENCRYPTED_PREFS) { open(LEGACY_ENCRYPTED_PREFS) } }
        } catch (e: Throwable) {
            context.getSharedPreferences("vitals_session_fallback", Context.MODE_PRIVATE).also {
                it.adoptLegacySession(LEGACY_FALLBACK_PREFS) {
                    context.getSharedPreferences(LEGACY_FALLBACK_PREFS, Context.MODE_PRIVATE)
                }
            }
        }
    }

    /**
     * Before the rename the session lived in "nexus_*" files. Move the token over once, so an
     * app update doesn't sign everyone out, then delete the old file (it holds the same token).
     * The file is checked on disk first: opening a missing one would create it.
     */
    private fun android.content.SharedPreferences.adoptLegacySession(
        legacyName: String,
        openLegacy: () -> android.content.SharedPreferences
    ) {
        val legacyFile = java.io.File(context.applicationInfo.dataDir, "shared_prefs/$legacyName.xml")
        if (!legacyFile.exists()) return
        try {
            val token = openLegacy().getString("auth_token", null)
            if (token != null && !contains("auth_token")) {
                edit().putString("auth_token", token).commit()
            }
            context.deleteSharedPreferences(legacyName)
        } catch (e: Exception) {
            // Unreadable (e.g. its key was lost): leave it; the user signs in again as before.
        }
    }

    private val _sessionToken = MutableStateFlow<String?>(getSessionToken())
    val sessionToken: Flow<String?> = _sessionToken.asStateFlow()

    // A stored value the keystore can no longer decrypt (key lost after a restore or a lock-screen
    // change) throws on read. That used to crash the app at start: treat it as signed out instead.
    fun getSessionToken(): String? = try {
        encryptedPrefs.getString("auth_token", null)
    } catch (t: Throwable) {
        android.util.Log.w("UserPreferencesRepo", "Stored session unreadable; signing out locally", t)
        try { encryptedPrefs.edit().remove("auth_token").apply() } catch (_: Throwable) {}
        null
    }

    fun saveSessionToken(token: String) {
        try {
            encryptedPrefs.edit().putString("auth_token", token).apply()
        } catch (t: Throwable) {
            android.util.Log.e("UserPreferencesRepo", "Could not save the session", t)
        }
        _sessionToken.value = token
    }

    fun clearSessionToken() {
        try {
            encryptedPrefs.edit().remove("auth_token").apply()
        } catch (t: Throwable) {
            android.util.Log.e("UserPreferencesRepo", "Could not clear the session", t)
        }
        _sessionToken.value = null
    }

    companion object {
        private const val LEGACY_ENCRYPTED_PREFS = "nexus_encrypted_session"
        private const val LEGACY_FALLBACK_PREFS = "nexus_session_fallback"
        val ONBOARDING_COMPLETED = booleanPreferencesKey("onboarding_completed")
        val IS_LOGGED_IN = booleanPreferencesKey("is_logged_in")
        val USER_ID = stringPreferencesKey("user_id")
        val USER_NAME = stringPreferencesKey("user_name")
        val USER_EMAIL = stringPreferencesKey("user_email")
        val TEMP_UNIT_FAHRENHEIT = booleanPreferencesKey("temp_unit_fahrenheit")
        val DARK_MODE = booleanPreferencesKey("dark_mode")
        val CPU_THRESHOLD = floatPreferencesKey("cpu_threshold")
        val GPU_THRESHOLD = floatPreferencesKey("gpu_threshold")
        val RAM_THRESHOLD = floatPreferencesKey("ram_threshold")
        val DISK_THRESHOLD = floatPreferencesKey("disk_threshold")
        val FCM_TOKEN = stringPreferencesKey("fcm_token")

        fun deviceCpuKey(deviceId: String) = floatPreferencesKey("cpu_thresh_$deviceId")
        fun deviceGpuKey(deviceId: String) = floatPreferencesKey("gpu_thresh_$deviceId")
        fun deviceRamKey(deviceId: String) = floatPreferencesKey("ram_thresh_$deviceId")
        fun deviceDiskKey(deviceId: String) = floatPreferencesKey("disk_thresh_$deviceId")
        fun deviceHasCustomKey(deviceId: String) = booleanPreferencesKey("has_custom_thresh_$deviceId")
    }

    val fcmToken: Flow<String?> = context.dataStore.data.map { prefs ->
        prefs[FCM_TOKEN]
    }

    suspend fun setFcmToken(token: String) {
        context.dataStore.edit { it[FCM_TOKEN] = token }
    }

    val onboardingCompleted: Flow<Boolean> = context.dataStore.data.map { prefs ->
        prefs[ONBOARDING_COMPLETED] ?: false
    }

    val isLoggedIn: Flow<Boolean> = context.dataStore.data.map { prefs ->
        val loggedIn = prefs[IS_LOGGED_IN] ?: false
        val token = getSessionToken()
        loggedIn && !token.isNullOrBlank()
    }

    val userName: Flow<String> = context.dataStore.data.map { prefs ->
        prefs[USER_NAME] ?: ""
    }

    val userEmail: Flow<String> = context.dataStore.data.map { prefs ->
        prefs[USER_EMAIL] ?: ""
    }

    val useFahrenheit: Flow<Boolean> = context.dataStore.data.map { prefs ->
        prefs[TEMP_UNIT_FAHRENHEIT] ?: false
    }

    val darkMode: Flow<Boolean> = context.dataStore.data.map { prefs ->
        prefs[DARK_MODE] ?: true
    }

    val cpuThreshold: Flow<Float> = context.dataStore.data.map { prefs ->
        prefs[CPU_THRESHOLD] ?: 90f
    }

    val gpuThreshold: Flow<Float> = context.dataStore.data.map { prefs ->
        prefs[GPU_THRESHOLD] ?: 85f
    }

    val ramThreshold: Flow<Float> = context.dataStore.data.map { prefs ->
        prefs[RAM_THRESHOLD] ?: 90f
    }

    val diskThreshold: Flow<Float> = context.dataStore.data.map { prefs ->
        prefs[DISK_THRESHOLD] ?: 10f
    }

    suspend fun setOnboardingCompleted(completed: Boolean) {
        context.dataStore.edit { it[ONBOARDING_COMPLETED] = completed }
    }

    suspend fun setLoggedIn(loggedIn: Boolean) {
        context.dataStore.edit { it[IS_LOGGED_IN] = loggedIn }
    }

    suspend fun saveUserInfo(id: String, name: String, email: String) {
        context.dataStore.edit { prefs ->
            prefs[USER_ID] = id
            prefs[USER_NAME] = name
            prefs[USER_EMAIL] = email
            prefs[IS_LOGGED_IN] = true
        }
        val token = fcmToken.first()
        if (!token.isNullOrBlank() && email.isNotBlank()) {
            syncPushTokenToConvex(email, token)
        }
    }

    suspend fun setUserName(name: String) {
        context.dataStore.edit { it[USER_NAME] = name }
    }

    suspend fun clearUserInfo() {
        clearSessionToken()
        context.dataStore.edit { prefs ->
            prefs.remove(USER_ID)
            prefs.remove(USER_NAME)
            prefs.remove(USER_EMAIL)
            prefs[IS_LOGGED_IN] = false
        }
    }

    suspend fun setUseFahrenheit(useFahrenheit: Boolean) {
        context.dataStore.edit { it[TEMP_UNIT_FAHRENHEIT] = useFahrenheit }
    }

    suspend fun setDarkMode(darkMode: Boolean) {
        context.dataStore.edit { it[DARK_MODE] = darkMode }
    }

    /** Saves on this phone and on the server. Returns false when the server sync failed. */
    suspend fun saveThresholds(cpu: Float, gpu: Float, ram: Float, disk: Float): Boolean {
        context.dataStore.edit { prefs ->
            prefs[CPU_THRESHOLD] = cpu
            prefs[GPU_THRESHOLD] = gpu
            prefs[RAM_THRESHOLD] = ram
            prefs[DISK_THRESHOLD] = disk
        }
        val email = userEmail.first()
        return email.isNotBlank() && syncThresholdsToConvex(email, cpu, gpu, ram, disk = disk)
    }

    fun getDeviceCpuThreshold(deviceId: String): Flow<Float> = context.dataStore.data.map { prefs ->
        prefs[deviceCpuKey(deviceId)] ?: prefs[CPU_THRESHOLD] ?: 90f
    }

    fun getDeviceGpuThreshold(deviceId: String): Flow<Float> = context.dataStore.data.map { prefs ->
        prefs[deviceGpuKey(deviceId)] ?: prefs[GPU_THRESHOLD] ?: 85f
    }

    fun getDeviceRamThreshold(deviceId: String): Flow<Float> = context.dataStore.data.map { prefs ->
        prefs[deviceRamKey(deviceId)] ?: prefs[RAM_THRESHOLD] ?: 90f
    }

    fun getDeviceDiskThreshold(deviceId: String): Flow<Float> = context.dataStore.data.map { prefs ->
        prefs[deviceDiskKey(deviceId)] ?: prefs[DISK_THRESHOLD] ?: 10f
    }

    fun hasCustomThresholds(deviceId: String): Flow<Boolean> = context.dataStore.data.map { prefs ->
        prefs[deviceHasCustomKey(deviceId)] ?: false
    }

    /** Saves on this phone and on the server. Returns false when the server sync failed. */
    suspend fun saveDeviceThresholds(deviceId: String, cpu: Float, gpu: Float, ram: Float, disk: Float): Boolean {
        context.dataStore.edit { prefs ->
            prefs[deviceCpuKey(deviceId)] = cpu
            prefs[deviceGpuKey(deviceId)] = gpu
            prefs[deviceRamKey(deviceId)] = ram
            prefs[deviceDiskKey(deviceId)] = disk
            prefs[deviceHasCustomKey(deviceId)] = true
        }
        val email = userEmail.first()
        return email.isNotBlank() && syncThresholdsToConvex(email, cpu, gpu, ram, deviceId = deviceId, disk = disk)
    }

    suspend fun resetDeviceThresholds(deviceId: String): Boolean {
        context.dataStore.edit { prefs ->
            prefs.remove(deviceCpuKey(deviceId))
            prefs.remove(deviceGpuKey(deviceId))
            prefs.remove(deviceRamKey(deviceId))
            prefs.remove(deviceDiskKey(deviceId))
            prefs.remove(deviceHasCustomKey(deviceId))
        }
        return resetDeviceThresholdsInConvex(deviceId)
    }

    suspend fun syncThresholdsToConvex(
        userEmail: String, cpu: Float, gpu: Float, ram: Float, deviceId: String? = null, disk: Float? = null
    ): Boolean {
        val token = getSessionToken()
        if (token.isNullOrBlank()) return false
        return withContext(Dispatchers.IO) {
            try {
                val args = JSONObject().apply {
                    put("token", token)
                    put("cpuTempC", cpu.toDouble())
                    put("gpuTempC", gpu.toDouble())
                    put("ramPct", ram.toDouble())
                    if (disk != null) put("diskFreePct", disk.toDouble())
                    if (!deviceId.isNullOrBlank()) {
                        put("deviceId", deviceId)
                    }
                }
                val req = ConvexHttp.request("mutation", "devices:updateThresholds", args)
                val ok = isConvexSuccess(req)
                android.util.Log.i("UserPreferencesRepo", "Threshold sync (device: $deviceId) ok=$ok")
                ok
            } catch (e: Exception) {
                android.util.Log.e("UserPreferencesRepo", "Failed to sync thresholds to Convex: ${e.message}")
                false
            }
        }
    }

    suspend fun resetDeviceThresholdsInConvex(deviceId: String): Boolean {
        val token = getSessionToken()
        if (token.isNullOrBlank() || deviceId.isBlank()) return false
        return withContext(Dispatchers.IO) {
            try {
                val req = ConvexHttp.request(
                    "mutation", "devices:resetDeviceThresholds",
                    JSONObject().put("token", token).put("deviceId", deviceId)
                )
                isConvexSuccess(req)
            } catch (e: Exception) {
                android.util.Log.e("UserPreferencesRepo", "Failed to reset thresholds in Convex: ${e.message}")
                false
            }
        }
    }

    suspend fun syncPushTokenToConvex(userEmail: String, fcmToken: String, phoneModel: String = android.os.Build.MODEL) {
        val token = getSessionToken()
        if (token.isNullOrBlank() || fcmToken.isBlank()) return
        withContext(Dispatchers.IO) {
            try {
                val req = ConvexHttp.request(
                    "mutation", "devices:registerPushToken",
                    JSONObject().put("token", token).put("fcmToken", fcmToken.trim()).put("phoneModel", phoneModel)
                )
                val body = httpClient.newCall(req).execute().use { it.body?.string().orEmpty() }
                if (JSONObject(body.ifBlank { "{}" }).optString("status") == "success") {
                    android.util.Log.i("UserPreferencesRepo", "Synced FCM push token to Convex for user")
                } else {
                    android.util.Log.w("UserPreferencesRepo", "Convex rejected the FCM push token: $body")
                }
            } catch (e: Exception) {
                android.util.Log.e("UserPreferencesRepo", "Failed to sync FCM push token to Convex: ${e.message}")
            }
        }
    }

    suspend fun resetThresholds(): Boolean = saveThresholds(90f, 85f, 90f, 10f)

    /** Values stored on the server (what alerts use), or null when unavailable. */
    data class ServerThresholds(val cpu: Float?, val gpu: Float?, val ram: Float?, val disk: Float?, val isCustom: Boolean)

    suspend fun fetchThresholdsFromConvex(deviceId: String?): ServerThresholds? {
        val token = getSessionToken()
        if (token.isNullOrBlank()) return null
        return withContext(Dispatchers.IO) {
            try {
                val args = JSONObject().put("token", token).apply {
                    if (!deviceId.isNullOrBlank()) put("deviceId", deviceId)
                }
                val req = ConvexHttp.request("query", "devices:getThresholds", args)
                val env = httpClient.newCall(req).execute().use { JSONObject(it.body?.string().orEmpty()) }
                val v = env.optJSONObject("value")
                if (env.optString("status") != "success" || v == null) return@withContext null
                fun num(k: String) = if (v.has(k) && !v.isNull(k)) v.getDouble(k).toFloat() else null
                ServerThresholds(num("cpuTempC"), num("gpuTempC"), num("ramPct"), num("diskFreePct"), v.optBoolean("isCustom", false))
            } catch (e: Exception) {
                null
            }
        }
    }

    /** Per-drive free-space limits for one device: drive name -> % free. Null when unavailable. */
    suspend fun fetchDiskThresholds(deviceId: String): Map<String, Float>? {
        val token = getSessionToken()
        if (token.isNullOrBlank()) return null
        return withContext(Dispatchers.IO) {
            try {
                val req = ConvexHttp.request(
                    "query", "devices:getDiskThresholds",
                    JSONObject().put("token", token).put("deviceId", deviceId)
                )
                val env = httpClient.newCall(req).execute().use { JSONObject(it.body?.string().orEmpty()) }
                val arr = env.optJSONArray("value")
                if (env.optString("status") != "success" || arr == null) return@withContext null
                buildMap {
                    for (i in 0 until arr.length()) {
                        val o = arr.optJSONObject(i) ?: continue
                        if (!o.isNull("minFreePct")) put(o.optString("drive"), o.getDouble("minFreePct").toFloat())
                    }
                }
            } catch (e: Exception) {
                null
            }
        }
    }

    /** Sets one drive's limit, or removes it (null) so the drive uses the default again. */
    suspend fun saveDiskThreshold(deviceId: String, drive: String, minFreePct: Float?): Boolean {
        val token = getSessionToken()
        if (token.isNullOrBlank()) return false
        return withContext(Dispatchers.IO) {
            try {
                val req = ConvexHttp.request(
                    "mutation", "devices:setDiskThreshold",
                    JSONObject()
                        .put("token", token)
                        .put("deviceId", deviceId)
                        .put("drive", drive)
                        .put("minFreePct", minFreePct?.toDouble() ?: JSONObject.NULL)
                )
                isConvexSuccess(req)
            } catch (e: Exception) {
                false
            }
        }
    }

    private fun isConvexSuccess(req: Request): Boolean =
        httpClient.newCall(req).execute().use { resp ->
            val body = resp.body?.string().orEmpty()
            resp.isSuccessful && body.isNotBlank() && JSONObject(body).optString("status") == "success"
        }
}

