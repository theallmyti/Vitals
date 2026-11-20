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

        // Global crash safeguard: log and prevent silent process crashes
        val defaultUncaughtHandler = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, throwable ->
            Log.e("VitalsApplication", "Uncaught exception on thread ${thread.name}: ${throwable.message}", throwable)
            defaultUncaughtHandler?.uncaughtException(thread, throwable)
        }

        try {
            NotificationHelper.createNotificationChannels(this)
        } catch (t: Throwable) {
            Log.w("VitalsApplication", "Notification channels creation failed", t)
        }

        try {
            com.pcmonitor.app.notification.AlertsSeen.init(this)
            com.pcmonitor.app.data.AvatarStore.init(this)
        } catch (t: Throwable) {
            Log.w("VitalsApplication", "State init failed", t)
        }

        userPreferencesRepository = UserPreferencesRepository(this)
        authRepository = MockAuthRepository(userPreferencesRepository)
        val devRepo = MockDeviceRepository(userPreferencesRepository, this)
        deviceRepository = devRepo
        alertRepository = MockAlertRepository(devRepo, userPreferencesRepository)
        historyRepository = MockHistoryRepository(userPreferencesRepository)

        // Fetch current FCM token (safely handle missing or placeholder Firebase config)
        try {
            FirebaseMessaging.getInstance().token.addOnCompleteListener { task ->
                if (task.isSuccessful) {
                    val token = task.result
                    Log.i("VitalsApplication", "FCM Registration Token: $token")
                    CoroutineScope(Dispatchers.IO).launch {
                        try {
                            userPreferencesRepository.setFcmToken(token)
                            val email = userPreferencesRepository.userEmail.first()
                            if (email.isNotBlank()) {
                                userPreferencesRepository.syncPushTokenToConvex(email, token)
                            }
                        } catch (t: Throwable) {
                            Log.w("VitalsApplication", "FCM token sync failed", t)
                        }
                    }
                } else {
                    Log.w("VitalsApplication", "Failed to fetch FCM registration token", task.exception)
                }
            }
        } catch (t: Throwable) {
            Log.w("VitalsApplication", "FirebaseMessaging not initialized: ${t.message}")
        }

        // Keep the home-screen widget in step with the app's device list (at most every 30 s).
        CoroutineScope(Dispatchers.Default).launch {
            try {
                var lastAt = 0L
                var lastCount = -1
                devRepo.devices.collect { list ->
                    val now = android.os.SystemClock.elapsedRealtime()
                    if (list.size != lastCount || now - lastAt >= 30_000L) {
                        lastAt = now
                        lastCount = list.size
                        try {
                            com.pcmonitor.app.widget.DevicesWidget.render(this@PCMonitorApplication, list)
                        } catch (t: Throwable) {
                            Log.w("VitalsApplication", "Widget render failed", t)
                        }
                    }
                }
            } catch (t: Throwable) {
                Log.w("VitalsApplication", "Widget collection failed", t)
            }
        }

        // Automatically start the realistic 1-second simulation loop
        CoroutineScope(Dispatchers.Default).launch {
            try {
                devRepo.startMockSimulation()
            } catch (t: Throwable) {
                Log.w("VitalsApplication", "Mock simulation failed", t)
            }
        }
    }
}
