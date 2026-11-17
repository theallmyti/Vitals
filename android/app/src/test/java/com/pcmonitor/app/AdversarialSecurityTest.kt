package com.pcmonitor.app

import com.pcmonitor.app.data.mock.MockAuthRepository
import com.pcmonitor.app.data.model.LoginRequest
import com.pcmonitor.app.data.model.RamStats
import com.pcmonitor.app.data.model.RegisterRequest
import kotlinx.coroutines.runBlocking
import org.junit.Assert.*
import org.junit.Test

class AdversarialSecurityTest {

    @Test
    fun testZeroRamTotalDivisionByZero() {
        val ramZero = RamStats(
            usedMb = 0L,
            totalMb = 0L,
            freeMb = 0L
        )
        // Must not crash with NaN or Infinity or divide by zero exception
        assertEquals(0f, ramZero.usagePercent, 0.001f)
    }

    @Test
    fun testExtremelyLargeValues() {
        val largeRam = RamStats(
            usedMb = Long.MAX_VALUE / 2,
            totalMb = Long.MAX_VALUE,
            freeMb = Long.MAX_VALUE / 2
        )
        assertTrue(largeRam.usagePercent.isFinite())
    }

    @Test
    fun testAuthAdversarialVectors() = runBlocking {
        val repo = MockAuthRepository(baseUrl = "")

        // No backdoor: the old built-in demo account must not sign in, and with no server nothing
        // signs in locally (accounts exist only on the server).
        val demoResult = repo.login(LoginRequest("  USER@EXAMPLE.COM  ", "password123"))
        assertTrue("Built-in demo account must not exist", demoResult.isFailure)
        assertTrue(
            "Offline sign-in explains the problem",
            demoResult.exceptionOrNull()?.message?.contains("Can't reach Vitals") == true
        )

        // Tier 7: Injection strings must fail gracefully without unhandled exception
        val sqliResult = repo.login(LoginRequest("' OR '1'='1", "password123"))
        assertTrue("SQLi attempt must fail gracefully", sqliResult.isFailure)

        val xssResult = repo.login(LoginRequest("<script>alert(1)</script>", "password123"))
        assertTrue("XSS attempt must fail gracefully", xssResult.isFailure)

        // Tier 7: Wrong credentials (email registered previously)
        val wrongPassResult = repo.login(LoginRequest("user@example.com", "wrongpassword"))
        assertTrue("Wrong password must fail", wrongPassResult.isFailure)

        // Tier 1 & 7: Malformed registration
        val shortPassResult = repo.register(RegisterRequest("User", "new@example.com", "123"))
        assertTrue("Password < 8 chars must fail", shortPassResult.isFailure)

        // Registration never creates a local-only account when the server can't be reached.
        val offlineRegister = repo.register(RegisterRequest("User", "new@example.com", "password123"))
        assertTrue("Offline registration must fail", offlineRegister.isFailure)

        val malformedEmailResult = repo.register(RegisterRequest("User", "not-an-email", "password123"))
        assertTrue("Malformed email must fail", malformedEmailResult.isFailure)
    }

    @Test
    fun testDeviceUnpairStateIntegrity() = runBlocking {
        val repo = com.pcmonitor.app.data.mock.MockDeviceRepository()
        val initialCount = repo.devices.value.size

        // Tier 1: Attempt to remove non-existent device ID - must be safe and not corrupt flow
        repo.removeDevice("non_existent_device_id_99999")
        assertEquals(initialCount, repo.devices.value.size)

        // Tier 1: Attempt to remove empty device ID - must be safe
        repo.removeDevice("")
        assertEquals(initialCount, repo.devices.value.size)
    }
}
