package com.pcmonitor.app

import com.pcmonitor.app.update.AppUpdater
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class AppUpdaterTest {

    @Test
    fun testCompareVersions() {
        assertEquals(0, AppUpdater.compareVersions("1.1.1", "1.1.1"))
        assertEquals(0, AppUpdater.compareVersions("1.0.0", "1.0.0"))
        assertEquals(0, AppUpdater.compareVersions("1.0", "1.0.0"))
        assertEquals(0, AppUpdater.compareVersions("2", "2.0.0"))

        assertTrue(AppUpdater.compareVersions("1.1.2", "1.1.1") > 0)
        assertTrue(AppUpdater.compareVersions("1.2.0", "1.1.9") > 0)
        assertTrue(AppUpdater.compareVersions("2.0.0", "1.9.9") > 0)

        assertTrue(AppUpdater.compareVersions("1.1.0", "1.1.1") < 0)
        assertTrue(AppUpdater.compareVersions("1.0.9", "1.1.0") < 0)
        assertTrue(AppUpdater.compareVersions("0.9.9", "1.0.0") < 0)
    }

    @Test
    fun testIsUpdateNeeded_sameVersionDoesNotPrompt() {
        // User's exact bug scenario:
        // App is already updated to 1.1.1 (versionCode 10101 or lower/default).
        // Backend published 1.1.1 with versionCode 10101.
        // It should NEVER prompt update!
        assertFalse(
            AppUpdater.isUpdateNeeded(
                releaseVersion = "1.1.1",
                releaseCode = 10101,
                installedVersion = "1.1.1",
                installedCode = 10101
            )
        )
        assertFalse(
            AppUpdater.isUpdateNeeded(
                releaseVersion = "1.1.1",
                releaseCode = 10101,
                installedVersion = "1.1.1",
                installedCode = 1 // e.g. debug build with default code
            )
        )
    }

    @Test
    fun testIsUpdateNeeded_olderOrEqualReleaseDoesNotPrompt() {
        // Release is 1.1.0, installed is 1.1.1
        assertFalse(
            AppUpdater.isUpdateNeeded(
                releaseVersion = "1.1.0",
                releaseCode = 10100,
                installedVersion = "1.1.1",
                installedCode = 10101
            )
        )
        // Release semver is older even if releaseCode was somehow higher
        assertFalse(
            AppUpdater.isUpdateNeeded(
                releaseVersion = "1.1.0",
                releaseCode = 20000,
                installedVersion = "1.1.1",
                installedCode = 10101
            )
        )
    }

    @Test
    fun testIsUpdateNeeded_strictlyNewerReleasePrompts() {
        // Valid update: 1.1.2 is newer than 1.1.1 and code 10102 > 10101
        assertTrue(
            AppUpdater.isUpdateNeeded(
                releaseVersion = "1.1.2",
                releaseCode = 10102,
                installedVersion = "1.1.1",
                installedCode = 10101
            )
        )
        assertTrue(
            AppUpdater.isUpdateNeeded(
                releaseVersion = "1.2.0",
                releaseCode = 10200,
                installedVersion = "1.1.1",
                installedCode = 10101
            )
        )
        assertTrue(
            AppUpdater.isUpdateNeeded(
                releaseVersion = "2.0.0",
                releaseCode = 20000,
                installedVersion = "1.1.1",
                installedCode = 10101
            )
        )
    }

    @Test
    fun testIsUpdateNeeded_codeNotHigherDoesNotPrompt() {
        // Release semver says 1.1.2, but releaseCode <= installedCode (Android installer would fail)
        assertFalse(
            AppUpdater.isUpdateNeeded(
                releaseVersion = "1.1.2",
                releaseCode = 10101,
                installedVersion = "1.1.1",
                installedCode = 10101
            )
        )
    }

    @Test
    fun testIsUpdateNeeded_blankInstalledVersionFallsBackToCode() {
        assertTrue(
            AppUpdater.isUpdateNeeded(
                releaseVersion = "1.1.1",
                releaseCode = 10101,
                installedVersion = null,
                installedCode = 10000
            )
        )
        assertFalse(
            AppUpdater.isUpdateNeeded(
                releaseVersion = "1.1.1",
                releaseCode = 10000,
                installedVersion = "",
                installedCode = 10000
            )
        )
    }
}
