import os
from typing import Optional, Dict

from .cargo_base import CargoHook
from ...db.base import DBConnection
from ...models.scm.git import GitClient
from ...models.scm.base import Repository


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
        self.build_options = build_options if build_options else []
        self.test_options = test_options if test_options else []

        if "--no-fail-fast" not in build_options:
            build_options += ["--no-fail-fast"]

    def env(self):
        return os.environ | self.env_vars

    def clean_command(self):
        return "cargo clean"

    def build_command(self):
        return self.test_command()

    def test_command(self):
        return "cargo test --tests --examples {0} -- {1}".format(
            " ".join(self.build_options),
            " ".join(self.test_options),
        )
