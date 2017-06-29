/**
 * Copyright (C) 2012 Typesafe, Inc. <http://www.typesafe.com>
 */

package org.pantsbuild.zinc

import java.io.File

import scala.compat.java8.OptionConverters._

import sbt.util.Level
import sbt.internal.inc.IncrementalCompilerImpl
import xsbti.CompileFailed
import org.pantsbuild.zinc.logging.Loggers

/**
 * Command-line main class.
 */
object Main {
  val Command     = "zinc"
  val Description = "scala incremental compiler"

  /**
   * Full zinc version info.
   */
  case class Version(published: String, timestamp: String, commit: String)

  /**
   * Get the zinc version from a generated properties file.
   */
  lazy val zincVersion: Version = {
    val props = Util.propertiesFromResource("zinc.version.properties", getClass.getClassLoader)
    Version(
      props.getProperty("version", "unknown"),
      props.getProperty("timestamp", ""),
      props.getProperty("commit", "")
    )
  }

  /**
   * For snapshots the zinc version includes timestamp and commit.
   */
  lazy val versionString: String = {
    import zincVersion._
    if (published.endsWith("-SNAPSHOT")) "%s %s-%s" format (published, timestamp, commit take 10)
    else published
  }

  /**
   * Print the zinc version to standard out.
   */
  def printVersion(): Unit = println("%s (%s) %s" format (Command, Description, versionString))

  /**
   * Run a compile.
   */
  def main(args: Array[String]): Unit = {
    val startTime = System.currentTimeMillis

    val Parsed(settings, residual, errors) = Settings.parse(args)

    val log = Loggers.create(settings.consoleLog.logLevel, settings.consoleLog.color)
    val isDebug = settings.consoleLog.logLevel <= Level.Debug

    // bail out on any command-line option errors
    if (errors.nonEmpty) {
      for (error <- errors) log.error(error)
      log.error("See %s -help for information about options" format Command)
      sys.exit(1)
    }

    if (settings.version) printVersion()

    if (settings.help) Settings.printUsage(Command)

    // if there are no sources provided, print outputs based on current analysis if requested,
    // else print version and usage by default
    if (settings.sources.isEmpty) {
      if (!settings.version && !settings.help) {
        printVersion()
        Settings.printUsage(Command)
        sys.exit(1)
      }
      sys.exit(0)
    }

    // Load the existing analysis for the destination, if any.
    val analysisMap = AnalysisMap.create(settings.cacheMap, settings.analysis.rebaseMap, log)
    val (targetAnalysisStore, previousResult) = analysisMap.loadDestinationAnalysis(settings, log)
    val inputs = InputUtils.create(settings, analysisMap, previousResult, log)

    if (isDebug) {
      log.debug(s"Inputs: $inputs")
    }

    try {
      // Run the compile.
      val result = new IncrementalCompilerImpl().compile(inputs, log)

      // Store the output if the result changed.
      if (result.hasModified) {
        targetAnalysisStore.set(result.analysis, result.setup)
      }

      log.info("Compile success " + Util.timing(startTime))
    } catch {
      case e: CompileFailed =>
        log.error("Compile failed " + Util.timing(startTime))
        sys.exit(1)
      case e: Exception =>
        if (isDebug) e.printStackTrace
        val message = e.getMessage
        if (message ne null) log.error(message)
        sys.exit(1)
    }
  }
}
