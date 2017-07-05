# coding=utf-8
# Copyright 2017 Pants project contributors (see CONTRIBUTORS.md).
# Licensed under the Apache License, Version 2.0 (see LICENSE).

from __future__ import (absolute_import, division, generators, nested_scopes, print_function,
                        unicode_literals, with_statement)

from pants.backend.jvm.subsystems.dependency_context import DependencyContext
from pants.backend.jvm.subsystems.jvm_tool_mixin import JvmToolMixin
from pants.backend.jvm.subsystems.shader import Shader
from pants.backend.jvm.tasks.classpath_util import ClasspathUtil
from pants.java.jar.jar_dependency import JarDependency
from pants.subsystem.subsystem import Subsystem
from pants.util.memo import memoized_method, memoized_property


class Zinc(Subsystem, JvmToolMixin):
  """Configuration for Pants' zinc wrapper tool."""

  options_scope = 'zinc'

  ZINC_COMPILE_MAIN = 'org.pantsbuild.zinc.Main'
  DEFAULT_CONFS = ['default']

  @classmethod
  def register_options(cls, register):
    super(Zinc, cls).register_options(register)
    Zinc.register_options_for(cls, register)

  @classmethod
  def subsystem_dependencies(cls):
    return super(Zinc, cls).subsystem_dependencies() + (DependencyContext,)

  @staticmethod
  def register_options_for(jvm_tool_mixin_cls, register, **kwargs):
    """Register options for the zinc tool in the context of the given JvmToolMixin.
    
    TODO: Move into the classmethod after zinc registration has been removed
    from `zinc_compile` in `1.6.0.dev0`.
    """
    cls = jvm_tool_mixin_cls

    register('--javac-plugins', advanced=True, type=list, fingerprint=True,
             help='Use these javac plugins.',
             **kwargs)
    register('--javac-plugin-args', advanced=True, type=dict, default={}, fingerprint=True,
             help='Map from javac plugin name to list of arguments for that plugin.',
             **kwargs)
    cls.register_jvm_tool(register, 'javac-plugin-dep', classpath=[],
                          help='Search for javac plugins here, as well as in any '
                               'explicit dependencies.',
                          **kwargs)

    register('--scalac-plugins', advanced=True, type=list, fingerprint=True,
             help='Use these scalac plugins.',
             **kwargs)
    register('--scalac-plugin-args', advanced=True, type=dict, default={}, fingerprint=True,
             help='Map from scalac plugin name to list of arguments for that plugin.',
             **kwargs)
    cls.register_jvm_tool(register, 'scalac-plugin-jars', classpath=[],
                          removal_version='1.5.0.dev0',
                          removal_hint='Use --compile-zinc-scalac-plugin-dep instead.')
    cls.register_jvm_tool(register, 'scalac-plugin-dep', classpath=[],
                          help='Search for scalac plugins here, as well as in any '
                               'explicit dependencies.',
                          **kwargs)

    def sbt_jar(name, **kwargs):
      return JarDependency(org='org.scala-sbt', name=name, rev='1.0.0-X16-SNAPSHOT-2', **kwargs)

    shader_rules = [
        # The compiler-interface and compiler-bridge tool jars carry xsbt and
        # xsbti interfaces that are used across the shaded tool jar boundary so
        # we preserve these root packages wholesale along with the core scala
        # APIs.
        Shader.exclude_package('scala', recursive=True),
        Shader.exclude_package('xsbt', recursive=True),
        Shader.exclude_package('xsbti', recursive=True),
      ]

    cls.register_jvm_tool(register,
                          'zinc',
                          classpath=[
                            JarDependency('org.pantsbuild', 'zinc_2.11', 'stuhood-zinc-1.0.0-X16-15'),
                          ],
                          **kwargs)

    cls.register_jvm_tool(register,
                          'compiler-bridge',
                          classpath=[
                            sbt_jar(name='compiler-bridge_2.11',
                                    classifier='sources',
                                    intransitive=True)
                          ],
                          **kwargs)
    cls.register_jvm_tool(register,
                          'compiler-interface',
                          classpath=[
                            sbt_jar(name='compiler-interface')
                          ],
                          # NB: We force a noop-jarjar'ing of the interface, since it is now broken
                          # up into multiple jars, but zinc does not yet support a sequence of jars
                          # for the interface.
                          main='no.such.main.Main',
                          custom_rules=shader_rules,
                          **kwargs)

  @memoized_method
  def _extra_compile_time_classpath_elements(self, jvm_tool_mixin_cls, products):
    """Any additional global compiletime classpath entries.

    TODO: Switch to memoized_property after 1.6.0.dev0.
    """
    javac_classpath = jvm_tool_mixin_cls.tool_classpath_from_products(products,
                                                                      'javac-plugin-dep',
                                                                      scope=self.options_scope)
    scalac_classpath = jvm_tool_mixin_cls.tool_classpath_from_products(products,
                                                                       'scalac-plugin-dep',
                                                                       scope=self.options_scope)
    classpaths = javac_classpath + scalac_classpath
    return [(conf, jar) for conf in self.DEFAULT_CONFS for jar in classpaths]

  def compile_classpath(self, jvm_tool_mixin_cls, products, classpath_product_key, target):
    """Compute the compile classpath for the given target."""
    classpath_product = products.get_data(classpath_product_key)

    if target.defaulted_property(lambda x: x.strict_deps):
      dependencies = DependencyContext.global_instance().strict_dependencies(target)
    else:
      dependencies = DependencyContext.global_instance().all_dependencies(target)
    extra_compile_time_classpath_elements = self._extra_compile_time_classpath_elements(jvm_tool_mixin_cls,
                                                                                        products)

    return ClasspathUtil.compute_classpath(dependencies,
                                           classpath_product,
                                           extra_compile_time_classpath_elements,
                                           self.DEFAULT_CONFS)
