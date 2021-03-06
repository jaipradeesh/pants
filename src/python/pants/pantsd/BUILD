# Copyright 2015 Pants project contributors (see CONTRIBUTORS.md).
# Licensed under the Apache License, Version 2.0 (see LICENSE).

python_library(
  name = 'process_manager',
  sources = ['process_manager.py'],
  dependencies = [
    '3rdparty/python:future',
    '3rdparty/python:psutil',
    'src/python/pants/base:build_environment',
    'src/python/pants/process',
    'src/python/pants/util:dirutil',
    'src/python/pants/util:process_handler',
  ]
)

python_library(
  name = 'pailgun_server',
  sources = ['pailgun_server.py'],
  dependencies = [
    '3rdparty/python:six',
    'src/python/pants/java:nailgun_protocol',
    'src/python/pants/util:contextutil',
    'src/python/pants/util:socket'
  ]
)

python_library(
  name = 'watchman',
  sources = ['watchman.py'],
  dependencies = [
    '3rdparty/python:future',
    'src/python/pants/util:dirutil',
    'src/python/pants/util:retry',
    ':process_manager',
    ':watchman_client'
  ]
)

python_library(
  name = 'watchman_launcher',
  sources = ['watchman_launcher.py'],
  dependencies = [
    '3rdparty/python:future',
    ':watchman',
    'src/python/pants/binaries',
    'src/python/pants/util:memo',
  ]
)

python_library(
  name = 'watchman_client',
  sources = ['watchman_client.py'],
  dependencies = [
    '3rdparty/python:pywatchman'
  ]
)

python_library(
  name = 'pants_daemon',
  sources = ['pants_daemon.py'],
  dependencies = [
    '3rdparty/python:future',
    '3rdparty/python:setproctitle',
    'src/python/pants/base:build_environment',
    'src/python/pants/base:exiter',
    'src/python/pants/binaries',
    'src/python/pants/engine:native',
    'src/python/pants/goal:run_tracker',
    'src/python/pants/init',
    'src/python/pants/pantsd/service:fs_event_service',
    'src/python/pants/pantsd/service:pailgun_service',
    'src/python/pants/pantsd/service:scheduler_service',
    'src/python/pants/pantsd/service:store_gc_service',
    'src/python/pants/util:collections',
    'src/python/pants/util:contextutil',
    'src/python/pants/util:memo',
    ':process_manager',
    ':watchman_launcher'
  ]
)
