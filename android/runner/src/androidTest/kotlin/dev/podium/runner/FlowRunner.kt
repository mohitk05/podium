package dev.podium.runner

import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.podium_core.*
import java.io.File

@RunWith(AndroidJUnit4::class)
class FlowRunner {
    @Test
    fun runFlows() {
        try {
            System.loadLibrary("podium_core")
            val version = coreVersion()
            Log.i("PODIUM", "Core version: $version")
            println("PODIUM|info|Core version: $version")

            val instrumentation = InstrumentationRegistry.getInstrumentation()
            val device = UiDevice.getInstance(instrumentation)
            val driver = UiAutomatorDriver(device)

            // Get flows from instrumentation arguments or assets
            val args = InstrumentationRegistry.getArguments()
            val flowsDir = args.getString("flowsDir")
            val flowBase64 = args.getString("flow")

            val flows = when {
                flowBase64 != null -> {
                    // Single flow passed as base64
                    val yaml = String(android.util.Base64.decode(flowBase64, android.util.Base64.DEFAULT))
                    listOf("inline" to yaml)
                }
                flowsDir != null -> {
                    // Load flows from device directory
                    val dir = File(flowsDir)
                    dir.listFiles { file -> file.extension == "yaml" || file.extension == "yml" }
                        ?.map { it.nameWithoutExtension to it.readText() }
                        ?: emptyList()
                }
                else -> {
                    // Load flows from assets
                    val assetManager = instrumentation.context.assets
                    try {
                        assetManager.list("flows")
                            ?.filter { it.endsWith(".yaml") || it.endsWith(".yml") }
                            ?.map {
                                val name = it.substringBeforeLast(".")
                                val yaml = assetManager.open("flows/$it").bufferedReader().use { reader ->
                                    reader.readText()
                                }
                                name to yaml
                            } ?: emptyList()
                    } catch (e: Exception) {
                        Log.w("PODIUM", "No flows found in assets")
                        emptyList()
                    }
                }
            }

            if (flows.isEmpty()) {
                Log.w("PODIUM", "No flows to run")
                println("PODIUM|warning|No flows found")
                return
            }

            val env = args.keySet()
                .filter { it.startsWith("env.") }
                .associate { key ->
                    val envKey = key.removePrefix("env.")
                    envKey to (args.getString(key) ?: "")
                }

            var allPassed = true

            for ((name, yaml) in flows) {
                Log.i("PODIUM", "Running flow: $name")
                println("PODIUM|flow|$name|started")

                try {
                    val flow = parseFlow(yaml, env)
                    val result = runFlow(flow, driver)

                    // Log each step
                    for (step in result.steps) {
                        val status = when (step.status) {
                            StepStatus.PASSED -> "passed"
                            StepStatus.FAILED -> "failed"
                            StepStatus.SKIPPED -> "skipped"
                        }
                        println("PODIUM|step|${step.commandDesc}|$status|${step.durationMs}ms")

                        if (step.status == StepStatus.FAILED) {
                            Log.e("PODIUM", "Step failed: ${step.commandDesc} - ${step.failureMessage}")
                        }
                    }

                    if (result.passed) {
                        Log.i("PODIUM", "Flow $name: PASSED")
                        println("PODIUM|flow|$name|passed")
                    } else {
                        Log.e("PODIUM", "Flow $name: FAILED")
                        println("PODIUM|flow|$name|failed")
                        allPassed = false

                        // Write hierarchy dump on failure
                        val resultsDir = File(instrumentation.targetContext.getExternalFilesDir(null), "podium/results")
                        resultsDir.mkdirs()
                        val hierarchyFile = File(resultsDir, "$name-hierarchy.xml")
                        device.dumpWindowHierarchy(hierarchyFile)
                    }

                    // Write result JSON
                    val resultsDir = File(instrumentation.targetContext.getExternalFilesDir(null), "podium/results")
                    resultsDir.mkdirs()
                    val resultFile = File(resultsDir, "$name.json")
                    resultFile.writeText(serializeFlowResult(result))

                } catch (e: Exception) {
                    Log.e("PODIUM", "Flow $name failed with exception", e)
                    println("PODIUM|flow|$name|error|${e.message}")
                    allPassed = false
                }
            }

            // Write JUnit XML
            writeJUnitXml(flows, allPassed)

            if (!allPassed) {
                throw AssertionError("One or more flows failed")
            }

        } catch (e: Exception) {
            Log.e("PODIUM", "Test execution failed", e)
            throw e
        }
    }

    private fun serializeFlowResult(result: FlowResult): String {
        // Simple JSON serialization
        val steps = result.steps.joinToString(",\n    ") { step ->
            """
            {
              "command": "${step.commandDesc.replace("\"", "\\\"")}",
              "status": "${step.status}",
              "duration_ms": ${step.durationMs},
              "failure_message": ${step.failureMessage?.let { "\"${it.replace("\"", "\\\"")}\"" } ?: "null"}
            }
            """.trimIndent()
        }
        return """
        {
          "passed": ${result.passed},
          "steps": [
            $steps
          ]
        }
        """.trimIndent()
    }

    private fun writeJUnitXml(flows: List<Pair<String, String>>, allPassed: Boolean) {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val resultsDir = File(instrumentation.targetContext.getExternalFilesDir(null), "podium/results")
        resultsDir.mkdirs()
        val junitFile = File(resultsDir, "junit.xml")

        val testCases = flows.map { (name, _) ->
            """<testcase name="$name" classname="dev.podium.runner.FlowRunner" />"""
        }.joinToString("\n    ")

        val xml = """
        <?xml version="1.0" encoding="UTF-8"?>
        <testsuite name="Podium Flows" tests="${flows.size}" failures="${if (allPassed) 0 else 1}">
          $testCases
        </testsuite>
        """.trimIndent()

        junitFile.writeText(xml)
    }
}

