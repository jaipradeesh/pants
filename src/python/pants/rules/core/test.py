# coding=utf-8
# Copyright 2018 Pants project contributors (see CONTRIBUTORS.md).
# Licensed under the Apache License, Version 2.0 (see LICENSE).

from __future__ import absolute_import, division, print_function, unicode_literals

from pants.base.exiter import PANTS_FAILED_EXIT_CODE, PANTS_SUCCEEDED_EXIT_CODE
from pants.build_graph.address import Address
from pants.engine.addressable import BuildFileAddresses
from pants.engine.console import Console
from pants.engine.goal import Goal
from pants.engine.legacy.graph import HydratedTarget
from pants.engine.rules import console_rule, rule
from pants.engine.selectors import Get
from pants.rules.core.core_test_model import Status, TestResult, TestTarget


class Test(Goal):
  """Runs tests."""

  name = 'test'


@console_rule(Test, [Console, BuildFileAddresses])
def fast_test(console, addresses):
  test_results = yield [Get(TestResult, Address, address.to_address()) for address in addresses]
  wrote_any_stdout = False
  did_any_fail = False
  for test_result in test_results:
    wrote_any_stdout |= bool(test_result.stdout)
    # Assume \n-terminated
    console.write_stdout(test_result.stdout)
    if test_result.stdout and not test_result.stdout[-1] == '\n':
      console.write_stdout('\n')
    if test_result.status == Status.FAILURE:
      did_any_fail = True

  if wrote_any_stdout:
    console.write_stdout('\n')

  for address, test_result in zip(addresses, test_results):
    console.print_stdout('{0:80}.....{1:>10}'.format(address.reference(), test_result.status))

  if did_any_fail:
    console.print_stderr('Tests failed')
    exit_code = PANTS_FAILED_EXIT_CODE
  else:
    exit_code = PANTS_SUCCEEDED_EXIT_CODE

  yield Test(exit_code)


@rule(TestResult, [HydratedTarget])
def coordinator_of_tests(target):
  # NB: This has the effect of "casting" a TargetAdaptor to a member of the TestTarget union. If the
  # TargetAdaptor is not a member of the union, it will fail at runtime with a useful error message.
  result = yield Get(TestResult, TestTarget, target.adaptor)
  yield result


def rules():
  return [
      coordinator_of_tests,
      fast_test,
    ]
