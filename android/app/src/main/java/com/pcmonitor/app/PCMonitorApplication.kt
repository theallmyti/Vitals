package com.pcmonitor.app

import android.app.Application
import android.util.Log
import com.google.firebase.messaging.FirebaseMessaging
import com.pcmonitor.app.data.mock.MockAlertRepository
import com.pcmonitor.app.data.mock.MockAuthRepository
import com.pcmonitor.app.data.mock.MockDeviceRepository
import com.pcmonitor.app.data.mock.MockHistoryRepository
import com.pcmonitor.app.data.repository.AlertRepository
import com.pcmonitor.app.data.repository.AuthRepository
import com.pcmonitor.app.data.repository.DeviceRepository
import com.pcmonitor.app.data.repository.HistoryRepository
import com.pcmonitor.app.data.repository.UserPreferencesRepository
import com.pcmonitor.app.notification.NotificationHelper
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

class PCMonitorApplication : Application() {

    lateinit var userPreferencesRepository: UserPreferencesRepository
        private set
    lateinit var authRepository: AuthRepository
        private set
    lateinit var deviceRepository: DeviceRepository
        private set
    lateinit var alertRepository: AlertRepository
        private set
    lateinit var historyRepository: HistoryRepository
        private set

    override fun onCreate() {
        super.onCreate()
        NotificationHelper.createNotificationChannels(this)
        com.pcmonitor.app.notification.AlertsSeen.init(this)
        com.pcmonitor.app.data.AvatarStore.init(this)

        userPreferencesRepository = UserPreferencesRepository(this)
        authRepository = MockAuthRepository(userPreferencesRepository)
        val devRepo = MockDeviceRepository(userPreferencesRepository, this)
        deviceRepository = devRepo
        alertRepository = MockAlertRepository(devRepo, userPreferencesRepository)
        historyRepository = MockHistoryRepository(userPreferencesRepository)

        // Fetch current FCM token. Guarded: a build without a usable Firebase config throws here,
        // and that must cost push alerts, not crash the app at every start.
        try {
            FirebaseMessaging.getInstance().token.addOnCompleteListener { task ->
                if (task.isSuccessful) {
                    val token = task.result
                    Log.i("VitalsApplication", "FCM Registration Token: $token")
                    CoroutineScope(Dispatchers.IO).launch {
                        userPreferencesRepository.setFcmToken(token)
                        val email = userPreferencesRepository.userEmail.first()
                        if (email.isNotBlank()) {
                            userPreferencesRepository.syncPushTokenToConvex(email, token)
                        }
                    }
                } else {
                    Log.w("VitalsApplication", "Failed to fetch FCM registration token", task.exception)
                }
            }
        } catch (t: Throwable) {
            Log.w("VitalsApplication", "Firebase Messaging unavailable; push alerts are off", t)
        }

        // Keep the home-screen widget in step with the app's device list (at most every 5 s, so it
        // follows the live numbers while the app is open; in the background it refreshes every
        // 15 min through DevicesWidget.RefreshWorker, the shortest period Android allows).
        CoroutineScope(Dispatchers.Default).launch {
            var lastAt = 0L
            var lastCount = -1
            devRepo.devices.collect { list ->
                val now = android.os.SystemClock.elapsedRealtime()
                if (list.size != lastCount || now - lastAt >= 5_000L) {
                    lastAt = now
                    lastCount = list.size
                    com.pcmonitor.app.widget.DevicesWidget.render(this@PCMonitorApplication, list)
                }
            }
        }

        // Automatically start the realistic 1-second simulation loop
        CoroutineScope(Dispatchers.Default).launch {
            devRepo.startMockSimulation()
        }
    }
}
