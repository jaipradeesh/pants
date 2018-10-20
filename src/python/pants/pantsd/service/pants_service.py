# coding=utf-8
# Copyright 2015 Pants project contributors (see CONTRIBUTORS.md).
# Licensed under the Apache License, Version 2.0 (see LICENSE).

from __future__ import absolute_import, division, print_function, unicode_literals

import threading
from abc import abstractmethod

from pants.util.meta import AbstractClass
from pants.util.objects import datatype


class PantsServices(datatype([
  # A tuple of instantiated PantsService instances.
  ('services', tuple),
  # A dict of (port_name -> port_info) for named ports hosted by the services.
  ('port_map', dict),
  # A lock to guard lifecycle changes for the services. This can be used by individual services
  # to safeguard daemon-synchronous sections that should be protected from abrupt teardown.
  # Notably, this lock is currently acquired for an entire pailgun request (by PailgunServer).
  # NB: This is a `threading.RLock` instance, but the constructor for RLock is an alias for a
  # native function, rather than an actual type.
  'lifecycle_lock',
])):
  """A registry of PantsServices instances"""


class PantsService(AbstractClass):
  """Pants daemon service base class."""

  class ServiceError(Exception): pass

  def __init__(self):
    super(PantsService, self).__init__()
    self.name = self.__class__.__name__
    self._kill_switch = threading.Event()

  @property
  def is_killed(self):
    """A `threading.Event`-checking property to facilitate graceful shutdown of services.

    Subclasses should check this property for a True value in their core runtime. If True, the
    service should teardown and gracefully exit. This represents a fatal/one-time event for the
    service.
    """
    return self._kill_switch.is_set()

  def setup(self, services):
    """Called before `run` to allow for service->service or other side-effecting setup.

    :param PantsServices services: A registry of all services within this run.
    """
    self.services = services

  @abstractmethod
  def run(self):
    """The main entry-point for the service called by the service runner."""

  def terminate(self):
    """Called upon service teardown."""
    self._kill_switch.set()
