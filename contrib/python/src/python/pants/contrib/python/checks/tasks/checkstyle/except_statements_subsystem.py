# coding=utf-8
# Copyright 2015 Pants project contributors (see CONTRIBUTORS.md).
# Licensed under the Apache License, Version 2.0 (see LICENSE).

from __future__ import absolute_import, division, print_function, unicode_literals

from pants.contrib.python.checks.tasks.checkstyle.plugin_subsystem_base import PluginSubsystemBase


class ExceptStatementsSubsystem(PluginSubsystemBase):
  options_scope = 'pycheck-except-statement'

  def get_plugin_type(self):
    from pants.contrib.python.checks.tasks.checkstyle.except_statements import ExceptStatements
    return ExceptStatements
