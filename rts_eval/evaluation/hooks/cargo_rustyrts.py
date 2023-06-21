import os
from enum import Enum
from typing import Optional, Dict

from .cargo_base import CargoHook
from ...db.base import DBConnection
from ...models.scm.git import GitClient
from ...models.scm.base import Repository

from os.path import abspath, expanduser


class RustyRTSMode(str, Enum):
    DYNAMIC = "dynamic"
    STATIC = "static"


class CargoRustyRTSHook(CargoHook):

    def __init__(self, repository: Repository,
                 git_client: GitClient,
                 mode: RustyRTSMode,
                 connection: DBConnection,
                 env_vars: Optional[Dict] = None,
                 build_options=None, rustc_options=None, test_options=None,
                 report_name: Optional[str] = None,
                 output_path: Optional[str] = None):
        super().__init__(repository, git_client, connection, report_name, output_path)

        self.target_dir = abspath(repository.path + "/target")
        self.mode = mode
        self.env_vars = env_vars
        self.build_options = build_options if build_options else []
        self.rustc_options = rustc_options if rustc_options else []
        self.test_options = test_options if test_options else []

    def env(self):
        return os.environ | self.env_vars

    def build_env(self):
        os.makedirs(self.target_dir + "/.rts_dynamic", exist_ok=True)

        env = {}
        if self.mode is RustyRTSMode.DYNAMIC:
            env["RUSTC_WRAPPER"] = abspath(expanduser("~/.cargo/bin/cargo-rustyrts"))
            env["RUSTYRTS_MODE"] = "dynamic"
            env["CARGO_TARGET_DIR"] = self.target_dir
            env["rustyrts_args"] = "[]"
        return os.environ | self.env_vars | env

    def clean_command(self):
        return "cargo clean"

    def build_command(self):
        return "cargo build --tests --examples {0}".format(
            " ".join(self.build_options)
        )

    def test_command_parent(self):
        return "cargo rustyrts {0} -- {1} -- {2} -- {1} -- {3}".format(
            self.mode,
            " ".join(self.build_options),
            " ".join(self.rustc_options),
            " ".join(self.test_options),
        )

    def test_command(self):
        return "cargo rustyrts {0} -v -- {1} -- {2} -- {1} -- {3}".format(
            self.mode,
            " ".join(self.build_options),
            " ".join(self.rustc_options),
            " ".join(self.test_options),
        )
