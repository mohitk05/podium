package dev.podium.runner

import android.content.Context
import android.content.Intent
import android.view.KeyEvent
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.BySelector
import androidx.test.uiautomator.Direction
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import uniffi.podium_core.Driver
import uniffi.podium_core.DriverException
import uniffi.podium_core.Selector
import java.io.File
import java.util.regex.Pattern

class UiAutomatorDriver(private val device: UiDevice) : Driver {
    private val context: Context = InstrumentationRegistry.getInstrumentation().targetContext

    override fun launchApp(appId: String, clearState: Boolean) {
        try {
            if (clearState) {
                device.executeShellCommand("pm clear $appId")
                Thread.sleep(500) // Wait for clear to complete
            }

            // Use shell command to launch
            val launchCmd = "monkey -p $appId -c android.intent.category.LAUNCHER 1"
            val result = device.executeShellCommand(launchCmd)

            if (result.contains("Error") || result.contains("No activities found")) {
                throw DriverException.OperationFailed(reason = "Failed to launch $appId: $result")
            }

            // Wait for app to start
            Thread.sleep(1000)
            device.waitForIdle(3000)
        } catch (e: DriverException) {
            throw e
        } catch (e: Exception) {
            throw DriverException.OperationFailed(reason = "Failed to launch $appId: ${e.message}")
        }
    }

    override fun isVisible(selector: Selector): Boolean {
        return try {
            val bySelector = selectorToBy(selector)
            device.hasObject(bySelector)
        } catch (e: Exception) {
            throw DriverException.OperationFailed(reason = "is_visible failed: ${e.message}")
        }
    }

    override fun tap(selector: Selector) {
        try {
            val bySelector = selectorToBy(selector)
            val obj = device.findObject(bySelector)
            if (obj == null) throw DriverException.ElementNotFound(reason = "Element not found: $selector")
            obj.click()
            device.waitForIdle(500)
        } catch (e: DriverException) {
            throw e
        } catch (e: Exception) {
            throw DriverException.OperationFailed(reason = "tap failed: ${e.message}")
        }
    }

    override fun inputText(text: String) {
        try {
            // Wait for an EditText to gain focus — proxy for the IME being ready.
            // Use a focused+class filter so we don't match non-editable focused views.
            val focused = device.wait(
                Until.findObject(By.focused(true).clazz("android.widget.EditText")),
                3000
            ) ?: throw DriverException.OperationFailed(reason = "No focused EditText within 3s")

            // Wait for the IME window to attach before calling setText; without this,
            // ACTION_SET_TEXT can silently no-op when the keyboard isn't up yet.
            Thread.sleep(300)

            focused.text = text

            // Dismiss keyboard so it doesn't cover elements in the next tap.
            // pressKeyCode is faster than executeShellCommand for key injection.
            device.pressKeyCode(KeyEvent.KEYCODE_ESCAPE)
            device.waitForIdle(300)
        } catch (e: DriverException) {
            throw e
        } catch (e: Exception) {
            throw DriverException.OperationFailed(reason = "inputText failed: ${e.message}")
        }
    }

    override fun swipe(direction: uniffi.podium_core.Direction) {
        try {
            val uiDirection = when (direction) {
                uniffi.podium_core.Direction.UP -> Direction.UP
                uniffi.podium_core.Direction.DOWN -> Direction.DOWN
                uniffi.podium_core.Direction.LEFT -> Direction.LEFT
                uniffi.podium_core.Direction.RIGHT -> Direction.RIGHT
            }

            // Prefer scrolling a scrollable container; fall back to raw swipe
            val scrollable = device.findObject(By.scrollable(true))
            if (scrollable != null) {
                scrollable.scroll(uiDirection, 0.8f)
            } else {
                val displayWidth = device.displayWidth
                val displayHeight = device.displayHeight
                val centerX = displayWidth / 2
                val centerY = displayHeight / 2
                val dist = (displayHeight * 0.4).toInt()
                when (uiDirection) {
                    Direction.UP -> device.swipe(centerX, centerY + dist, centerX, centerY - dist, 20)
                    Direction.DOWN -> device.swipe(centerX, centerY - dist, centerX, centerY + dist, 20)
                    Direction.LEFT -> device.swipe(centerX + dist, centerY, centerX - dist, centerY, 20)
                    Direction.RIGHT -> device.swipe(centerX - dist, centerY, centerX + dist, centerY, 20)
                }
            }

            Thread.sleep(300) // Wait for scroll animation
        } catch (e: DriverException) {
            throw e
        } catch (e: Exception) {
            throw DriverException.OperationFailed(reason = "swipe failed: ${e.message}")
        }
    }

    override fun back() {
        try {
            device.pressBack()
            Thread.sleep(200)
        } catch (e: Exception) {
            throw DriverException.OperationFailed(reason = "back failed: ${e.message}")
        }
    }

    override fun waitForIdle(timeoutMs: ULong) {
        try {
            device.waitForIdle(timeoutMs.toLong())
        } catch (e: Exception) {
            throw DriverException.Timeout(reason = "waitForIdle timed out after ${timeoutMs}ms")
        }
    }

    override fun takeScreenshot(name: String) {
        try {
            val screenshotDir = File(context.getExternalFilesDir(null), "podium/screenshots")
            screenshotDir.mkdirs()

            val screenshotFile = File(screenshotDir, "$name.png")
            device.takeScreenshot(screenshotFile)
        } catch (e: Exception) {
            throw DriverException.OperationFailed(reason = "takeScreenshot failed: ${e.message}")
        }
    }

    override fun nowMs(): ULong {
        return System.currentTimeMillis().toULong()
    }

    override fun sleepMs(ms: ULong) {
        Thread.sleep(ms.toLong())
    }

    private fun selectorToBy(selector: Selector): BySelector {
        val text = selector.text
        val id = selector.id

        // Priority: id > text (most specific first)
        if (id != null) {
            // Resource ID suffix match
            return By.res(Pattern.compile(".*:id/$id"))
        }

        if (text != null) {
            // Check if it's a regex pattern
            return if (text.startsWith("/") && text.endsWith("/")) {
                val pattern = text.substring(1, text.length - 1)
                By.text(Pattern.compile(pattern))
            } else {
                By.text(text)
            }
        }

        // Fallback (shouldn't happen)
        return By.text("")
    }
}
