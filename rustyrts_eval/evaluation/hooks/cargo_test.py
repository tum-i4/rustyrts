import logging
import os
from os.path import abspath
from typing import Optional, Dict

from .cargo_base import CargoHook
from ...db.base import DBConnection
from ...models.scm.git import GitClient
from ...models.scm.base import Repository
from ...util.logging.logger import get_logger

_LOGGER = get_logger(__name__)

class CargoTestHook(CargoHook):

    def __init__(self, repository: Repository,
                 git_client: GitClient,
                 connection: DBConnection,
                 env_vars: Optional[Dict] = None,
                 build_options=None, test_options=None,
                 report_name: Optional[str] = None,
                 output_path: Optional[str] = None):
        super().__init__(repository, git_client, connection, report_name, output_path)

        self.env_vars = env_vars
        self.target_dir = abspath(repository.path + "/target_test")
        self.build_options = build_options if build_options else []
        self.test_options = test_options if test_options else []

    def env(self):
        os.makedirs(self.target_dir, exist_ok=True)
        env = {"CARGO_TARGET_DIR": self.target_dir}
        return os.environ | self.env_vars | env

    def build_env(self):
        return os.environ | self.env_vars

    def clean_command(self):
        return "cargo clean"

    def build_command(self, features):
        build_options = " ".join(self.build_options) + (" --features {0}".format(features) if features else "")
        return "cargo build --all-targets {0}".format(build_options)

    def test_command_parent(self, features):
        return self.test_command(features)

    def test_command(self, features):
        build_options = " ".join(self.build_options) + (" --features {0}".format(features) if features else "")
        return "cargo test --tests --examples {0} --no-fail-fast -- {1}".format(
            build_options,
            " ".join(self.test_options),
        )
