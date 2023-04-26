# -*- coding: utf-8 -*-

try:
    from setuptools import setup
except ImportError:
    from distutils.core import setup

readme = ''

setup(
    long_description=readme,
    name='rts_eval',
    version='0.1',
    python_requires='==3.*,>=3.6.5',
    entry_points={"console_scripts": ["rts_eval = rts_eval.cli.cli:entry_point"]},
    packages=['rts_eval',
              'rts_eval.cli',
              'rts_eval.cli.db',
              'rts_eval.db',
              'rts_eval.models',
              'rts_eval.models.scm',
              'rts_eval.models.testing',
              'rts_eval.models.testing.loaders',
              'rts_eval.util',
              'rts_eval.util.logging',
              'rts_eval.util.os',
              'rts_eval.util.scm',
              'rts_eval.evaluation',
              'rts_eval.evaluation.hooks'
              ],
    package_dir={"": "."},
    package_data={},
    install_requires=['click==7.*,>=7.1.2',
                      'gitpython==3.*,>=3.1.3',
                      'halo==0.*,>=0.0.29',
                      'sqlalchemy==1.*,>=1.3.17',
                      'psycopg2-binary==2.*,>=2.8.6'
                      ],
)
