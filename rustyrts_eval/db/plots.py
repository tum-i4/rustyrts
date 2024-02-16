import pandas as pd
import seaborn as sns
import matplotlib as mpl
import matplotlib.pyplot as plt
from sqlalchemy.sql import Select, distinct, literal_column, select
from sqlalchemy.sql.functions import coalesce, count, sum, aggregate_strings

from .analysis import get_test_diff, get_test_diff_and_intersection
from .git import DBCommit, DBRepository


class HistoryPlotter:
    def __init__(
        self, connection, view_info, output_format, sequential_watermark=False
    ):
        self.connection = connection
        self.view_info = view_info
        self.output_format = output_format
        self.sequential_watermark = sequential_watermark

        self.labels = view_info.get_labels(connection)

    def plot_history_duration_absolute(self):
        y_label = "absolute e2e testing time [s]"
        file = "duration_absolute" + self.output_format

        commit = DBCommit.__table__
        testreport_extended = self.view_info.testreport_extended

        durations = (
            select(
                commit.c.repo_id.label("repository"),
                testreport_extended.c.retest_all_test_duration,
                testreport_extended.c.dynamic_test_duration,
                testreport_extended.c.static_test_duration,
            )
            .select_from(testreport_extended, commit)
            .where(commit.c.id == testreport_extended.c.commit)
            .order_by(testreport_extended.c.commit)
        )

        df = self.connection.query(durations)

        df_retest_all = df[["repository"]].copy()
        df_retest_all["y"] = df["retest_all_test_duration"]
        df_retest_all["algorithm"] = "retest_all"

        df_dynamic = df[["repository"]].copy()
        df_dynamic["y"] = df["dynamic_test_duration"]
        df_dynamic["algorithm"] = "dynamic"

        df_static = df[["repository"]].copy()
        df_static["y"] = df["static_test_duration"]
        df_static["algorithm"] = "static"

        df = pd.concat([df_retest_all, df_dynamic, df_static])

        boxplot(
            df,
            self.labels["path"],
            y_label,
            file,
            ["#DAD7CB", "#E37222", "#A2AD00"],
            sequential_watermark=self.sequential_watermark,
        )

    def plot_history_duration_relative(self):
        y_label = "relative e2e testing time [%]"
        file = "duration_relative" + self.output_format

        commit = DBCommit.__table__
        testreport_extended = self.view_info.testreport_extended

        durations = (
            select(
                commit.c.repo_id.label("repository"),
                (
                    testreport_extended.c.dynamic_test_duration
                    * 100.0
                    / testreport_extended.c.retest_all_test_duration
                ).label("dynamic_test_duration"),
                (
                    testreport_extended.c.static_test_duration
                    * 100.0
                    / testreport_extended.c.retest_all_test_duration
                ).label("static_test_duration"),
            )
            .select_from(testreport_extended, commit)
            .where(commit.c.id == testreport_extended.c.commit)
            .order_by(testreport_extended.c.commit)
        )

        df = self.connection.query(durations)

        df_dynamic = df[["repository"]].copy()
        df_dynamic["y"] = df["dynamic_test_duration"]
        df_dynamic["algorithm"] = "dynamic"

        df_static = df[["repository"]].copy()
        df_static["y"] = df["static_test_duration"]
        df_static["algorithm"] = "static"

        df = pd.concat([df_dynamic, df_static])

        boxplot(
            df,
            self.labels["path"],
            y_label,
            file,
            ["#E37222", "#A2AD00"],
            sequential_watermark=self.sequential_watermark,
        )

    def plot_history_efficiency_repo(self):
        reg_label = "linear regression x~log(y)"
        y_label = "average relative e2e testing time [%]"
        x_label = "average absolute e2e testing time of retest-all [s]"
        file = "efficiency_repo"

        duration = self.view_info.duration

        efficiency = select(
            duration.c.repo_id.label("repository"),
            (100.0 * duration.c.retest_all_mean).label("retest_all_mean"),
            (100.0 * duration.c.dynamic_mean_relative).label("dynamic_mean_relative"),
            (100.0 * duration.c.static_mean_relative).label("static_mean_relative"),
        ).select_from(duration)

        df = self.connection.query(efficiency)

        df_dynamic = df[["repository"]].copy()
        df_dynamic["x"] = df["retest_all_mean"]
        df_dynamic["y"] = df["dynamic_mean_relative"]
        df_dynamic["algorithm"] = "dynamic"

        df_static = df[["repository"]].copy()
        df_static["x"] = df["retest_all_mean"]
        df_static["y"] = df["static_mean_relative"]
        df_static["algorithm"] = "static"

        scatterplot(
            [df_dynamic, df_static],
            ["dynamic - " + reg_label, "static - " + reg_label],
            x_label,
            y_label,
            file + self.output_format,
            ["#E37222", "#A2AD00"],
            sequential_watermark=self.sequential_watermark,
        )

        scatterplot(
            [df_dynamic, df_static],
            ["dynamic - " + reg_label, "static - " + reg_label],
            x_label,
            y_label,
            file + "_with_regression" + self.output_format,
            ["#E37222", "#A2AD00"],
            regression=True,
            sequential_watermark=self.sequential_watermark,
        )

    def plot_history_target_count_absolute(self):
        y_label = "absolute number of tests"
        file = "selected_targets_absolute"

        commit = DBCommit.__table__
        target_count = self.view_info.target_count

        count_unit = (
            select(
                commit.c.repo_id.label("repository"),
                target_count.c.retest_all_count,
                target_count.c.dynamic_count,
                target_count.c.static_count,
            )
            .select_from(target_count, commit)
            .where(target_count.c.commit == commit.c.id)
            .where(target_count.c.target == "UNIT")
            .order_by(target_count.c.commit)
        )
        count_integration = (
            select(
                commit.c.repo_id.label("repository"),
                target_count.c.retest_all_count,
                target_count.c.dynamic_count,
                target_count.c.static_count,
            )
            .select_from(target_count, commit)
            .where(target_count.c.commit == commit.c.id)
            .where(target_count.c.target == "INTEGRATION")
            .order_by(target_count.c.commit)
        )

        df_unit = self.connection.query(count_unit)
        df_integration = self.connection.query(count_integration)

        df_retest_all_unit = df_unit[["repository"]].copy()
        df_retest_all_unit["y"] = df_unit["retest_all_count"]
        df_retest_all_unit["algorithm"] = "retest_all - unit"

        df_dynamic_unit = df_unit[["repository"]].copy()
        df_dynamic_unit["y"] = df_unit["dynamic_count"]
        df_dynamic_unit["algorithm"] = "dynamic - unit"

        df_static_unit = df_unit[["repository"]].copy()
        df_static_unit["y"] = df_unit["static_count"]
        df_static_unit["algorithm"] = "static - unit"

        df_retest_all_integration = df_integration[["repository"]].copy()
        df_retest_all_integration["y"] = df_integration["retest_all_count"]
        df_retest_all_integration["algorithm"] = "retest_all - integration"

        df_dynamic_integration = df_integration[["repository"]].copy()
        df_dynamic_integration["y"] = df_integration["dynamic_count"]
        df_dynamic_integration["algorithm"] = "dynamic - integration"

        df_static_integration = df_integration[["repository"]].copy()
        df_static_integration["y"] = df_integration["static_count"]
        df_static_integration["algorithm"] = "static - integration"

        df_dynamic = pd.concat(
            [
                df_retest_all_unit,
                df_retest_all_integration,
                df_dynamic_unit,
                df_dynamic_integration,
            ]
        )
        df_static = pd.concat(
            [
                df_retest_all_unit,
                df_retest_all_integration,
                df_static_unit,
                df_static_integration,
            ]
        )

        boxplot(
            df_dynamic,
            self.labels["path"],
            y_label,
            file + "_dynamic" + self.output_format,
            ["#E0DED4", "#ADABA1", "#E98C4A", "#B65C1B"],
            sequential_watermark=self.sequential_watermark,
        )
        boxplot(
            df_static,
            self.labels["path"],
            y_label,
            file + "_static" + self.output_format,
            ["#E0DED4", "#ADABA1", "#B4BE26", "#818B00"],
            sequential_watermark=self.sequential_watermark,
        )
        # boxplot(df, labels, y_label, file, ["#DAD7CB", "#E37222", "#A2AD00"])

    def plot_history_target_count_relative(self):
        y_label = "relative number of tests [%]"
        file = "selected_targets_relative"

        commit = DBCommit.__table__
        target_count = self.view_info.target_count

        count_unit = (
            select(
                commit.c.repo_id.label("repository"),
                (
                    target_count.c.dynamic_count
                    * 100.0
                    / target_count.c.retest_all_count
                ).label("dynamic_count"),
                (
                    target_count.c.static_count
                    * 100.0
                    / target_count.c.retest_all_count
                ).label("static_count"),
            )
            .select_from(target_count, commit)
            .where(target_count.c.commit == commit.c.id)
            .where(target_count.c.target == "UNIT")
            .order_by(target_count.c.commit)
        )
        count_integration = (
            select(
                commit.c.repo_id.label("repository"),
                (
                    target_count.c.dynamic_count
                    * 100.0
                    / target_count.c.retest_all_count
                ).label("dynamic_count"),
                (
                    target_count.c.static_count
                    * 100.0
                    / target_count.c.retest_all_count
                ).label("static_count"),
            )
            .select_from(target_count, commit)
            .where(target_count.c.commit == commit.c.id)
            .where(target_count.c.target == "INTEGRATION")
            .order_by(target_count.c.commit)
        )

        df_unit = self.connection.query(count_unit)
        df_integration = self.connection.query(count_integration)

        df_dynamic_unit = df_unit[["repository"]].copy()
        df_dynamic_unit["y"] = df_unit["dynamic_count"]
        df_dynamic_unit["algorithm"] = "dynamic - unit"

        df_static_unit = df_unit[["repository"]].copy()
        df_static_unit["y"] = df_unit["static_count"]
        df_static_unit["algorithm"] = "static - unit"

        df_dynamic_integration = df_integration[["repository"]].copy()
        df_dynamic_integration["y"] = df_integration["dynamic_count"]
        df_dynamic_integration["algorithm"] = "dynamic - integration"

        df_static_integration = df_integration[["repository"]].copy()
        df_static_integration["y"] = df_integration["static_count"]
        df_static_integration["algorithm"] = "static - integration"

        df = pd.concat(
            [
                df_dynamic_unit,
                df_dynamic_integration,
                df_static_unit,
                df_static_integration,
            ]
        )

        boxplot_with_observations(
            df,
            self.labels["path"],
            y_label,
            file + self.output_format,
            ["#E98C4A", "#B65C1B", "#B4BE26", "#818B00"],
            sequential_watermark=self.sequential_watermark,
            figsize=(24, 15),
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )
        boxplot(
            df,
            self.labels["path"],
            y_label,
            file + "_boxplot" + self.output_format,
            ["#E98C4A", "#B65C1B", "#B4BE26", "#818B00"],
            sequential_watermark=self.sequential_watermark,
            figsize=(24, 15),
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )
        stripplot(
            df,
            self.labels["path"],
            y_label,
            file + "_stripplot" + self.output_format,
            ["#E98C4A", "#B65C1B", "#B4BE26", "#818B00"],
            sequential_watermark=self.sequential_watermark,
            figsize=(24, 15),
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )

    def plot_history_testcases_contains_relation(self, partition=False):
        y_label = "Tests that have been selected"
        file = "contains_all_tests"

        commit = DBCommit.__table__
        testcases_selected = self.view_info.testcases_selected

        selected = (
            select(
                commit.c.repo_id.label("repository"),
                testcases_selected.c.commit,
                testcases_selected.c.dynamic,
                testcases_selected.c.static,
            )
            .select_from(testcases_selected, commit)
            .where(testcases_selected.c.commit == commit.c.id)
            .order_by(testcases_selected.c.commit)
        )

        df = self.connection.query(selected)

        df_selected_dynamic = df[["repository", "dynamic", "commit"]]
        df_selected_static = df[["repository", "static", "commit"]]

        not_selected_static = []

        selected_dynamic = df_selected_dynamic.to_dict(orient="records")
        selected_static = df_selected_static.to_dict(orient="records")

        assert len(selected_static) == len(selected_static)

        for dynamic_report, static_report in zip(selected_dynamic, selected_static):
            assert dynamic_report["commit"] == static_report["commit"]

            repository = static_report["repository"]
            commit = static_report["commit"]

            diff = get_test_diff(dynamic_report["dynamic"], static_report["static"])

            not_selected_static.append(
                {
                    "repository": repository,
                    "commit": commit,
                    "algorithm": "dynamic but not static",
                    "y": len(diff),
                }
            )

        df_not_selected_static = pd.DataFrame(not_selected_static)
        df = pd.concat([df_not_selected_static[["repository", "algorithm", "y"]]])

        if partition:
            filter_normal = [1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12]
            filter_special = [8]

            labels1 = self.labels["path"][:7] + self.labels["path"][8:]
            labels2 = [self.labels["path"][7]]

            df_1 = df[(df["repository"].isin(filter_normal))]
            df_2 = df[(df["repository"].isin(filter_special))]

            stripplot(
                df_1,
                labels1,
                y_label,
                file + "_1" + self.output_format,
                ["#E37222"],
                hue="algorithm",
                figsize=(18, 15),
                legend_anchor=(0.3, 0.9, 0.7, 0.1),
                sequential_watermark=self.sequential_watermark,
            )
            stripplot(
                df_2,
                labels2,
                "",
                file + "_2" + self.output_format,
                ["#E37222"],
                hue="algorithm",
                figsize=(3, 15),
                legend=False,
            )
        else:
            stripplot(
                df,
                self.labels["path"],
                y_label,
                file + self.output_format,
                ["#E37222"],
                hue="algorithm",
                figsize=(18, 15),
                legend_anchor=(0.3, 0.9, 0.7, 0.1),
                sequential_watermark=self.sequential_watermark,
            )

    def plot_history_testcases_count_absolute(self):
        y_label = "absolute number of tests"
        file = "selected_tests_absolute" + self.output_format

        commit = DBCommit.__table__
        testcases_count = self.view_info.testcases_count

        count = (
            select(
                commit.c.repo_id.label("repository"),
                testcases_count.c.retest_all_count,
                testcases_count.c.dynamic_count,
                testcases_count.c.static_count,
            )
            .select_from(testcases_count, commit)
            .where(commit.c.id == testcases_count.c.commit)
            .order_by(testcases_count.c.commit)
        )

        df = self.connection.query(count)

        df_retest_all = df[["repository"]].copy()
        df_retest_all["y"] = df["retest_all_count"]
        df_retest_all["algorithm"] = "retest_all"

        df_dynamic = df[["repository"]].copy()
        df_dynamic["y"] = df["dynamic_count"]
        df_dynamic["algorithm"] = "dynamic"

        df_static = df[["repository"]].copy()
        df_static["y"] = df["static_count"]
        df_static["algorithm"] = "static"

        df = pd.concat([df_retest_all, df_dynamic, df_static])

        boxplot(
            df,
            self.labels["path"],
            y_label,
            file,
            ["#DAD7CB", "#E37222", "#A2AD00"],
            sequential_watermark=self.sequential_watermark,
        )

    def plot_history_testcases_count_relative(self):
        y_label = "relative number of tests [%]"
        file = "selected_tests_relative"

        commit = DBCommit.__table__
        testcases_count = self.view_info.testcases_count

        count = (
            select(
                commit.c.repo_id.label("repository"),
                (
                    testcases_count.c.dynamic_count
                    * 100.0
                    / testcases_count.c.retest_all_count
                ).label("dynamic_count"),
                (
                    testcases_count.c.static_count
                    * 100.0
                    / testcases_count.c.retest_all_count
                ).label("static_count"),
            )
            .select_from(testcases_count, commit)
            .where(commit.c.id == testcases_count.c.commit)
            .order_by(testcases_count.c.commit)
        )

        df = self.connection.query(count)

        df_dynamic = df[["repository"]].copy()
        df_dynamic["y"] = df["dynamic_count"]
        df_dynamic["algorithm"] = "dynamic"

        df_static = df[["repository"]].copy()
        df_static["y"] = df["static_count"]
        df_static["algorithm"] = "static"

        df = pd.concat([df_dynamic, df_static])

        boxplot_with_observations(
            df,
            self.labels["path"],
            y_label,
            file + self.output_format,
            ["#E37222", "#A2AD00"],
            sequential_watermark=self.sequential_watermark,
            figsize=(22, 15),
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )
        boxplot(
            df,
            self.labels["path"],
            y_label,
            file + "_boxplot" + self.output_format,
            ["#E37222", "#A2AD00"],
            sequential_watermark=self.sequential_watermark,
            figsize=(22, 15),
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )
        stripplot(
            df,
            self.labels["path"],
            y_label,
            file + "_stripplot" + self.output_format,
            ["#E37222", "#A2AD00"],
            sequential_watermark=self.sequential_watermark,
            figsize=(22, 15),
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )

    def plot_history_testcases_different_absolute(self):
        y_label_selected = "Tests with different result, selected"
        file_selected = "different_and_selected_absolute" + self.output_format

        y_label_not_selected = "Tests with different result, not selected"
        file_not_selected = "different_and_not_selected_absolute" + self.output_format

        commit = DBCommit.__table__
        testcases_selected = self.view_info.testcases_selected
        testcases_different = self.view_info.testcases_different

        different_retest_all = (
            select(
                commit.c.repo_id.label("repository"),
                testcases_different.c.commit,
                testcases_different.c.retest_all,
            )
            .select_from(testcases_different, commit)
            .where(commit.c.id == testcases_different.c.commit)
            .order_by(testcases_different.c.commit)
        )
        selected = (
            select(
                commit.c.repo_id.label("repository"),
                testcases_selected.c.commit,
                testcases_selected.c.dynamic,
                testcases_selected.c.static,
            )
            .select_from(testcases_selected, commit)
            .where(commit.c.id == testcases_selected.c.commit)
            .order_by(testcases_selected.c.commit)
        )

        df_different_retest_all = self.connection.query(different_retest_all)
        df_selected_rustyrts = self.connection.query(selected)

        df_selected_dynamic = df_selected_rustyrts[
            ["repository", "commit", "dynamic"]
        ].copy()

        df_selected_static = df_selected_rustyrts[
            ["repository", "commit", "static"]
        ].copy()

        selected_dynamic = []
        not_selected_dynamic = []
        selected_static = []
        not_selected_static = []

        different_retest_all_count = {}

        raw_different_retest_all = df_different_retest_all.to_dict(orient="records")
        raw_selected_dynamic = df_selected_dynamic.to_dict(orient="records")
        raw_selected_static = df_selected_static.to_dict(orient="records")

        assert len(raw_different_retest_all) == len(raw_selected_dynamic) and len(
            raw_different_retest_all
        ) == len(raw_selected_static)

        for retest_all_report, dynamic_report, static_report in zip(
            raw_different_retest_all, raw_selected_dynamic, raw_selected_static
        ):
            repository = retest_all_report["repository"]
            commit = retest_all_report["commit"]

            if repository not in different_retest_all_count:
                different_retest_all_count[repository] = {}
                different_retest_all_count[repository]["count"] = 0
                different_retest_all_count[repository]["commits"] = 0
            count = len(set(retest_all_report["retest_all"].splitlines()))
            if count > 0:
                different_retest_all_count[repository]["count"] += count
                different_retest_all_count[repository]["commits"] += 1

            (diff_dynamic, intersection_dynamic) = get_test_diff_and_intersection(
                retest_all_report["retest_all"], dynamic_report["dynamic"]
            )
            (diff_static, intersection_static) = get_test_diff_and_intersection(
                retest_all_report["retest_all"], static_report["static"]
            )

            selected_dynamic.append(
                {
                    "repository": repository,
                    "commit": commit,
                    "algorithm": "dynamic",
                    "y": len(intersection_dynamic),
                }
            )
            not_selected_dynamic.append(
                {
                    "repository": repository,
                    "commit": commit,
                    "algorithm": "dynamic",
                    "y": len(diff_dynamic),
                }
            )
            selected_static.append(
                {
                    "repository": repository,
                    "commit": commit,
                    "algorithm": "static",
                    "y": len(intersection_static),
                }
            )
            not_selected_static.append(
                {
                    "repository": repository,
                    "commit": commit,
                    "algorithm": "static",
                    "y": len(diff_static),
                }
            )

        df_selected_dynamic = pd.DataFrame(selected_dynamic)
        df_selected_static = pd.DataFrame(selected_static)

        df_not_selected_dynamic = pd.DataFrame(not_selected_dynamic)
        df_not_selected_static = pd.DataFrame(not_selected_static)

        labels = []
        for i in range(len(self.labels)):
            labels.append(
                self.labels["path"][i]
                + (
                    # "\n(" +
                    " - "
                    + str(different_retest_all_count[i + 1]["count"])
                    + " on "
                    + str(different_retest_all_count[i + 1]["commits"])
                    # + ")"
                )
            )

        df_selected = pd.concat(
            [
                df_selected_dynamic[["repository", "algorithm", "y"]],
                df_selected_static[["repository", "algorithm", "y"]],
            ]
        )

        df_not_selected = pd.concat(
            [
                df_not_selected_dynamic[["repository", "algorithm", "y"]],
                df_not_selected_static[["repository", "algorithm", "y"]],
            ]
        )

        stripplot(
            df_selected,
            labels,
            y_label_selected,
            file_selected,
            ["#E37222", "#A2AD00"],
            hue="algorithm",
            sequential_watermark=self.sequential_watermark,
        )
        stripplot(
            df_not_selected,
            labels,
            y_label_not_selected,
            file_not_selected,
            ["#E37222", "#A2AD00"],
            hue="algorithm",
            sequential_watermark=self.sequential_watermark,
        )


########################################################################################################################
# Mutants plots


class MutantsPlotter:
    def __init__(self, connection, view_info, output_format):
        super().__init__()
        self.connection = connection
        self.view_info = view_info
        self.output_format = output_format

        self.labels = view_info.get_labels(connection)

    def plot_mutants_duration_absolute(self):
        y_label = "absolute e2e testing time [s]"
        file = "duration_absolute" + self.output_format

        mutant_extended = self.view_info.mutant_extended

        durations = (
            select(
                mutant_extended.c.commit.label("repository"),
                mutant_extended.c.retest_all_test_duration,
                mutant_extended.c.dynamic_test_duration,
                mutant_extended.c.static_test_duration,
            )
            .select_from(mutant_extended)
            .order_by(mutant_extended.c.commit)
        )

        df = self.connection.query(durations)

        df_retest_all = df[["repository"]].copy()
        df_retest_all["y"] = df["retest_all_test_duration"]
        df_retest_all["algorithm"] = "retest_all"

        df_dynamic = df[["repository"]].copy()
        df_dynamic["y"] = df["dynamic_test_duration"]
        df_dynamic["algorithm"] = "dynamic"

        df_static = df[["repository"]].copy()
        df_static["y"] = df["static_test_duration"]
        df_static["algorithm"] = "static"

        df = pd.concat([df_retest_all, df_dynamic, df_static])

        boxplot(
            df, self.labels["path"], y_label, file, ["#DAD7CB", "#E37222", "#A2AD00"]
        )

    def plot_mutants_duration_relative(self):
        y_label = "relative e2e testing time [%]"
        file = "duration_relative" + self.output_format

        mutant_extended = self.view_info.mutant_extended

        durations = (
            select(
                mutant_extended.c.commit.label("repository"),
                (
                    mutant_extended.c.dynamic_test_duration
                    * 100.0
                    / mutant_extended.c.retest_all_test_duration
                ).label("dynamic_test_duration"),
                (
                    mutant_extended.c.static_test_duration
                    * 100.0
                    / mutant_extended.c.retest_all_test_duration
                ).label("static_test_duration"),
            )
            .select_from(mutant_extended)
            .order_by(mutant_extended.c.commit)
        )

        df = self.connection.query(durations)

        df_dynamic = df[["repository"]].copy()
        df_dynamic["y"] = df["dynamic_test_duration"]
        df_dynamic["algorithm"] = "dynamic"

        df_static = df[["repository"]].copy()
        df_static["y"] = df["static_test_duration"]
        df_static["algorithm"] = "static"

        df = pd.concat([df_dynamic, df_static])

        boxplot(df, self.labels["path"], y_label, file, ["#E37222", "#A2AD00"])

    def plot_mutants_target_count_absolute(self):
        y_label = "absolute number of tests"
        file = "selected_targets_absolute"

        target_count = self.view_info.target_count

        count_unit = (
            select(
                target_count.c.commit.label("repository"),
                target_count.c.retest_all_count,
                target_count.c.dynamic_count,
                target_count.c.static_count,
            )
            .select_from(target_count)
            .where(target_count.c.target == "UNIT")
            .order_by(target_count.c.commit)
        )
        count_integration = (
            select(
                target_count.c.commit.label("repository"),
                target_count.c.retest_all_count,
                target_count.c.dynamic_count,
                target_count.c.static_count,
            )
            .select_from(target_count)
            .where(target_count.c.target == "INTEGRATION")
            .order_by(target_count.c.commit)
        )

        df_unit = self.connection.query(count_unit)
        df_integration = self.connection.query(count_integration)

        df_retest_all_unit = df_unit[["repository"]].copy()
        df_retest_all_unit["y"] = df_unit["retest_all_count"]
        df_retest_all_unit["algorithm"] = "retest_all - unit"

        df_dynamic_unit = df_unit[["repository"]].copy()
        df_dynamic_unit["y"] = df_unit["dynamic_count"]
        df_dynamic_unit["algorithm"] = "dynamic - unit"

        df_static_unit = df_unit[["repository"]].copy()
        df_static_unit["y"] = df_unit["static_count"]
        df_static_unit["algorithm"] = "static - unit"

        df_retest_all_integration = df_integration[["repository"]].copy()
        df_retest_all_integration["y"] = df_integration["retest_all_count"]
        df_retest_all_integration["algorithm"] = "retest_all - integration"

        df_dynamic_integration = df_integration[["repository"]].copy()
        df_dynamic_integration["y"] = df_integration["dynamic_count"]
        df_dynamic_integration["algorithm"] = "dynamic - integration"

        df_static_integration = df_integration[["repository"]].copy()
        df_static_integration["y"] = df_integration["static_count"]
        df_static_integration["algorithm"] = "static - integration"

        df_dynamic = pd.concat(
            [
                df_retest_all_unit,
                df_retest_all_integration,
                df_dynamic_unit,
                df_dynamic_integration,
            ]
        )
        df_static = pd.concat(
            [
                df_retest_all_unit,
                df_retest_all_integration,
                df_static_unit,
                df_static_integration,
            ]
        )

        boxplot(
            df_dynamic,
            self.labels["path"],
            y_label,
            file + "_dynamic" + self.output_format,
            ["#E0DED4", "#ADABA1", "#E98C4A", "#B65C1B"],
        )
        boxplot(
            df_static,
            self.labels["path"],
            y_label,
            file + "_static" + self.output_format,
            ["#E0DED4", "#ADABA1", "#B4BE26", "#818B00"],
        )

        # boxplot(df, labels, y_label, file, ["#DAD7CB", "#E37222", "#A2AD00"])

    def plot_mutants_target_count_relative(self):
        y_label = "relative number of tests [%]"
        file = "selected_targets_relative"

        target_count = self.view_info.target_count

        count_unit = (
            select(
                target_count.c.commit.label("repository"),
                (
                    target_count.c.dynamic_count
                    * 100.0
                    / target_count.c.retest_all_count
                ).label("dynamic_count"),
                (
                    target_count.c.static_count
                    * 100.0
                    / target_count.c.retest_all_count
                ).label("static_count"),
            )
            .select_from(target_count)
            .where(target_count.c.target == "UNIT")
            .order_by(target_count.c.commit)
        )
        count_integration = (
            select(
                target_count.c.commit.label("repository"),
                (
                    target_count.c.dynamic_count
                    * 100.0
                    / target_count.c.retest_all_count
                ).label("dynamic_count"),
                (
                    target_count.c.static_count
                    * 100.0
                    / target_count.c.retest_all_count
                ).label("static_count"),
            )
            .select_from(target_count)
            .where(target_count.c.target == "INTEGRATION")
            .order_by(target_count.c.commit)
        )

        df_unit = self.connection.query(count_unit)
        df_integration = self.connection.query(count_integration)

        df_dynamic_unit = df_unit[["repository"]].copy()
        df_dynamic_unit["y"] = df_unit["dynamic_count"]
        df_dynamic_unit["algorithm"] = "dynamic - unit"

        df_static_unit = df_unit[["repository"]].copy()
        df_static_unit["y"] = df_unit["static_count"]
        df_static_unit["algorithm"] = "static - unit"

        df_dynamic_integration = df_integration[["repository"]].copy()
        df_dynamic_integration["y"] = df_integration["dynamic_count"]
        df_dynamic_integration["algorithm"] = "dynamic - integration"

        df_static_integration = df_integration[["repository"]].copy()
        df_static_integration["y"] = df_integration["static_count"]
        df_static_integration["algorithm"] = "static - integration"

        df = pd.concat(
            [
                df_dynamic_unit,
                df_dynamic_integration,
                df_static_unit,
                df_static_integration,
            ]
        )

        boxplot_with_observations(
            df,
            self.labels["path"],
            y_label,
            file + self.output_format,
            ["#E98C4A", "#B65C1B", "#B4BE26", "#818B00"],
            figsize=(22, 15),
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )
        boxplot(
            df,
            self.labels["path"],
            y_label,
            file + "_boxplot" + self.output_format,
            ["#E98C4A", "#B65C1B", "#B4BE26", "#818B00"],
            figsize=(24, 15),
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )
        stripplot(
            df,
            self.labels["path"],
            y_label,
            file + "_stripplot" + self.output_format,
            ["#E98C4A", "#B65C1B", "#B4BE26", "#818B00"],
            figsize=(24, 15),
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )

    def plot_mutants_testcases_contains_relation(self):
        y_label = "Tests that have been selected"
        file = "contains_all_tests" + self.output_format

        testcases_selected = self.view_info.testcases_selected

        selected = (
            select(
                testcases_selected.c.commit.label("repository"),
                testcases_selected.c.retest_all_mutant_id,
                testcases_selected.c.dynamic,
                testcases_selected.c.static,
                testcases_selected.c.descr.label("mutant"),
            )
            .select_from(testcases_selected)
            .where(testcases_selected.c.descr != "baseline")
            .order_by(testcases_selected.c.commit)
        )

        df = self.connection.query(selected)

        df_selected_dynamic = df[
            ["repository", "retest_all_mutant_id", "dynamic", "mutant"]
        ]
        df_selected_static = df[
            ["repository", "retest_all_mutant_id", "static", "mutant"]
        ]

        not_selected_static = []

        selected_dynamic = df_selected_dynamic.to_dict(orient="records")
        selected_static = df_selected_static.to_dict(orient="records")

        assert len(selected_static) == len(selected_static)

        for dynamic_mutant, static_mutant in zip(selected_dynamic, selected_static):
            assert (
                dynamic_mutant["retest_all_mutant_id"]
                == static_mutant["retest_all_mutant_id"]
            )

            repository = static_mutant["repository"]
            descr = static_mutant["mutant"]

            diff = get_test_diff(dynamic_mutant["dynamic"], static_mutant["static"])

            not_selected_static.append(
                {
                    "repository": repository,
                    "mutant": descr,
                    "algorithm": "dynamic but not static",
                    "y": len(diff),
                }
            )

        df_not_selected_static = pd.DataFrame(not_selected_static)

        df = pd.concat([df_not_selected_static[["repository", "algorithm", "y"]]])

        stripplot(
            df,
            self.labels["path"],
            y_label,
            file,
            ["#E37222"],
            hue="algorithm",
            legend_loc="upper left",
            legend_anchor=(0.5, 0.7, 0.4, 0.3),
        )

    def plot_mutants_testcases_count_absolute(self):
        y_label = "absolute number of tests"
        file = "selected_tests_absolute" + self.output_format

        testcases_count = self.view_info.testcases_count

        count = (
            select(
                testcases_count.c.commit.label("repository"),
                testcases_count.c.retest_all_count,
                testcases_count.c.dynamic_count,
                testcases_count.c.static_count,
            )
            .select_from(testcases_count)
            .order_by(testcases_count.c.commit)
        )

        df = self.connection.query(count)

        df_retest_all = df[["repository"]].copy()
        df_retest_all["y"] = df["retest_all_count"]
        df_retest_all["algorithm"] = "retest_all"

        df_dynamic = df[["repository"]].copy()
        df_dynamic["y"] = df["dynamic_count"]
        df_dynamic["algorithm"] = "dynamic"

        df_static = df[["repository"]].copy()
        df_static["y"] = df["static_count"]
        df_static["algorithm"] = "static"

        df = pd.concat([df_retest_all, df_dynamic, df_static])

        boxplot(
            df, self.labels["path"], y_label, file, ["#DAD7CB", "#E37222", "#A2AD00"]
        )

    def plot_mutants_testcases_count_relative(self):
        y_label = "relative number of tests [%]"
        file = "selected_tests_relative"

        testcases_count = self.view_info.testcases_count

        count = (
            select(
                testcases_count.c.commit.label("repository"),
                (
                    testcases_count.c.dynamic_count
                    * 100.0
                    / testcases_count.c.retest_all_count
                ).label("dynamic_count"),
                (
                    testcases_count.c.static_count
                    * 100.0
                    / testcases_count.c.retest_all_count
                ).label("static_count"),
            )
            .select_from(testcases_count)
            .order_by(testcases_count.c.commit)
        )

        df = self.connection.query(count)

        df_dynamic = df[["repository"]].copy()
        df_dynamic["y"] = df["dynamic_count"]
        df_dynamic["algorithm"] = "dynamic"

        df_static = df[["repository"]].copy()
        df_static["y"] = df["static_count"]
        df_static["algorithm"] = "static"

        df = pd.concat([df_dynamic, df_static])

        boxplot_with_observations(
            df,
            self.labels["path"],
            y_label,
            file + self.output_format,
            ["#E37222", "#A2AD00"],
            figsize=(22, 15),
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )
        boxplot(
            df,
            self.labels["path"],
            y_label,
            file + "_boxplot" + self.output_format,
            ["#E37222", "#A2AD00"],
            figsize=(22, 15),
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )
        stripplot(
            df,
            self.labels["path"],
            y_label,
            file + "_stripplot" + self.output_format,
            ["#E37222", "#A2AD00"],
            figsize=(22, 15),
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )

    def plot_mutants_testcases_failed_absolute(self, partition=False):
        y_label_selected = "Failed tests, selected"
        file_selected = "failed_and_selected_absolute"

        y_label_not_selected = "Failed tests, not selected"
        file_not_selected = "failed_and_not_selected_absolute"

        testcases_selected = self.view_info.testcases_selected
        testcases_failed = self.view_info.testcases_failed

        failed_retest_all = (
            select(
                testcases_failed.c.commit.label("repository"),
                testcases_failed.c.retest_all_mutant_id,
                testcases_failed.c.descr.label("mutant"),
                testcases_failed.c.retest_all,
            )
            .select_from(testcases_failed)
            .where(testcases_failed.c.descr != "baseline")
            .order_by(testcases_failed.c.commit, testcases_failed.c.descr)
        )
        selected = (
            select(
                testcases_selected.c.commit.label("repository"),
                testcases_selected.c.retest_all_mutant_id,
                testcases_selected.c.descr.label("mutant"),
                testcases_selected.c.dynamic,
                testcases_selected.c.static,
            )
            .select_from(testcases_selected)
            .where(testcases_selected.c.descr != "baseline")
            .order_by(testcases_selected.c.commit, testcases_selected.c.descr)
        )

        df_failed_retest_all = self.connection.query(failed_retest_all)
        df_selected_rustyrts = self.connection.query(selected)

        df_selected_dynamic = df_selected_rustyrts[
            ["repository", "retest_all_mutant_id", "dynamic"]
        ].copy()

        df_selected_static = df_selected_rustyrts[
            ["repository", "retest_all_mutant_id", "static"]
        ].copy()

        selected_dynamic = []
        not_selected_dynamic = []
        selected_static = []
        not_selected_static = []

        raw_failed_retest_all = df_failed_retest_all.to_dict(orient="records")
        raw_selected_dynamic = df_selected_dynamic.to_dict(orient="records")
        raw_selected_static = df_selected_static.to_dict(orient="records")

        assert len(raw_failed_retest_all) == len(raw_selected_dynamic) and len(
            raw_failed_retest_all
        ) == len(raw_selected_static)

        for retest_all_mutant, dynamic_mutant, static_mutant in zip(
            raw_failed_retest_all, raw_selected_dynamic, raw_selected_static
        ):
            assert (
                retest_all_mutant["retest_all_mutant_id"]
                == dynamic_mutant["retest_all_mutant_id"]
            )
            assert (
                retest_all_mutant["retest_all_mutant_id"]
                == static_mutant["retest_all_mutant_id"]
            )

            repository = retest_all_mutant["repository"]
            descr = retest_all_mutant["mutant"]

            (diff_dynamic, intersection_dynamic) = get_test_diff_and_intersection(
                retest_all_mutant["retest_all"], dynamic_mutant["dynamic"]
            )
            (diff_static, intersection_static) = get_test_diff_and_intersection(
                retest_all_mutant["retest_all"], static_mutant["static"]
            )

            selected_dynamic.append(
                {
                    "repository": repository,
                    "mutant": descr,
                    "algorithm": "dynamic",
                    "y": len(intersection_dynamic),
                }
            )
            not_selected_dynamic.append(
                {
                    "repository": repository,
                    "mutant": descr,
                    "algorithm": "dynamic",
                    "y": len(diff_dynamic),
                }
            )
            selected_static.append(
                {
                    "repository": repository,
                    "mutant": descr,
                    "algorithm": "static",
                    "y": len(intersection_static),
                }
            )
            not_selected_static.append(
                {
                    "repository": repository,
                    "mutant": descr,
                    "algorithm": "static",
                    "y": len(diff_static),
                }
            )

        df_selected_dynamic = pd.DataFrame(selected_dynamic)
        df_not_selected_dynamic = pd.DataFrame(not_selected_dynamic)
        df_selected_static = pd.DataFrame(selected_static)
        df_not_selected_static = pd.DataFrame(not_selected_static)

        df_selected = pd.concat(
            [
                df_selected_dynamic[["repository", "algorithm", "y"]],
                df_selected_static[["repository", "algorithm", "y"]],
            ]
        )
        df_not_selected = pd.concat(
            [
                df_not_selected_dynamic[["repository", "algorithm", "y"]],
                df_not_selected_static[["repository", "algorithm", "y"]],
            ]
        )

        dfs = []
        if partition:
            filter_normal = [1, 2, 3, 5, 6, 7, 8, 9, 10]
            filter_special = [4]

            labels1 = self.labels[:3] + self.labels[4:]
            labels2 = [self.labels[3]]

            df_not_selected_1 = df_not_selected[
                (df_not_selected["repository"].isin(filter_normal))
            ]
            df_not_selected_2 = df_not_selected[
                (df_not_selected["repository"].isin(filter_special))
            ]
            stripplot(
                df_not_selected_1,
                labels1,
                y_label_not_selected,
                file_not_selected + "_1" + self.output_format,
                ["#E37222", "#A2AD00"],
                hue="algorithm",
                figsize=(18, 15),
                legend_anchor=(0.1, 0.9, 0.2, 0.1),
            )
            stripplot(
                df_not_selected_2,
                labels2,
                "",
                file_not_selected + "_2" + self.output_format,
                ["#E37222", "#A2AD00"],
                hue="algorithm",
                figsize=(3, 15),
                legend=False,
            )

        else:
            stripplot(
                df_not_selected,
                self.labels["path"],
                "",
                file_not_selected + self.output_format,
                ["#E37222", "#A2AD00"],
                hue="algorithm",
                figsize=(17, 15),
                legend=False,
            )

        stripplot(
            df_selected,
            self.labels["path"],
            y_label_selected,
            file_selected + self.output_format,
            ["#E37222", "#A2AD00"],
            hue="algorithm",
            figsize=(17, 15),
        )

    def plot_mutants_percentage_failed(self):
        y_label = "failed tests of selected tests [%]"
        file = "selected_tests_percentage_failed"

        testcases_count = self.view_info.testcases_count

        count_retest_all = (
            select(
                testcases_count.c.commit.label("repository"),
                (
                    testcases_count.c.retest_all_count_failed
                    * 100.0
                    / testcases_count.c.retest_all_count
                ).label("y"),
            )
            .select_from(testcases_count)
            .where(testcases_count.c.retest_all_count != 0)
            .order_by(testcases_count.c.commit)
        )
        count_dynamic = (
            select(
                testcases_count.c.commit.label("repository"),
                (
                    testcases_count.c.dynamic_count_failed
                    * 100.0
                    / testcases_count.c.dynamic_count
                ).label("y"),
            )
            .select_from(testcases_count)
            .where(testcases_count.c.dynamic_count != 0)
            .order_by(testcases_count.c.commit)
        )
        count_static = (
            select(
                testcases_count.c.commit.label("repository"),
                (
                    testcases_count.c.static_count_failed
                    * 100.0
                    / testcases_count.c.static_count
                ).label("y"),
            )
            .select_from(testcases_count)
            .where(testcases_count.c.static_count != 0)
            .order_by(testcases_count.c.commit)
        )

        df_retest_all = self.connection.query(count_retest_all)
        df_dynamic = self.connection.query(count_dynamic)
        df_static = self.connection.query(count_static)

        df_dynamic["algorithm"] = "dynamic"
        df_static["algorithm"] = "static"

        df = pd.concat([df_retest_all, df_dynamic, df_static])

        boxplot(
            df,
            self.labels["path"],
            y_label,
            file + self.output_format,
            ["#DAD7CB", "#E37222", "#A2AD00"],
            figsize=(22, 15),
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )


########################################################################################################################
# Plotting utilities


def boxplot(
    df,
    labels,
    y_label,
    file,
    palette=None,
    hue="algorithm",
    figsize=(20, 15),
    legend=True,
    legend_loc="best",
    legend_anchor=None,
    sequential_watermark=False,
):
    sns.set_style("whitegrid")
    sns.set_context("talk", font_scale=2.0)
    plt.figure(figsize=figsize)
    ax = sns.boxplot(
        data=df,
        x="repository",
        y="y",
        hue=hue,
        showmeans=True,
        width=0.75,
        meanprops={
            "marker": "v",
            "markerfacecolor": "white",
            "markeredgecolor": "black",
            "markersize": "16",
        },
        fliersize=14,
        palette=palette,
    )
    ax.set_xticklabels(labels=labels, rotation="vertical")
    ax.set_xlabel("")
    ax.set_ylabel(y_label)
    ax.get_yaxis().set_minor_locator(mpl.ticker.AutoMinorLocator())
    ax.grid(which="major", linewidth=1.0)
    ax.grid(which="minor", linewidth=0.5)
    if sequential_watermark:
        plt.figtext(0.01, 0.02, "single-threaded", color="grey", rotation="vertical")
    if legend:
        plt.legend(loc=legend_loc, bbox_to_anchor=legend_anchor)
    else:
        plt.legend([], [], frameon=False)
    plt.tight_layout(pad=0.2)
    plt.savefig(file)


def boxplot_with_observations(
    df,
    labels,
    y_label,
    file,
    palette=None,
    hue="algorithm",
    figsize=(20, 15),
    legend=True,
    legend_loc="best",
    legend_anchor=None,
    sequential_watermark=False,
):
    sns.set_style("whitegrid")
    sns.set_context("talk", font_scale=2.0)
    plt.figure(figsize=figsize)
    ax = sns.boxplot(
        data=df,
        x="repository",
        y="y",
        hue=hue,
        showmeans=True,
        width=0.75,
        meanprops={
            "marker": "v",
            "markerfacecolor": "white",
            "markeredgecolor": "black",
            "markersize": "16",
        },
        fliersize=14,
        palette=palette,
    )

    sns.stripplot(
        ax=ax,
        data=df,
        x="repository",
        y="y",
        hue=hue,
        dodge=True,
        jitter=0.3,
        size=8,
        linewidth=1,
        palette=palette,
        legend=False,
    )

    ax.set_xticklabels(labels=labels, rotation="vertical")
    ax.set_xlabel("")
    ax.set_ylabel(y_label)
    ax.get_yaxis().set_minor_locator(mpl.ticker.AutoMinorLocator())
    ax.grid(which="major", linewidth=1.0)
    ax.grid(which="minor", linewidth=0.5)
    if sequential_watermark:
        plt.figtext(0.01, 0.02, "single-threaded", color="grey", rotation="vertical")
    if legend:
        plt.legend(loc=legend_loc, bbox_to_anchor=legend_anchor)
    else:
        plt.legend([], [], frameon=False)
    plt.tight_layout(pad=0.2)
    plt.savefig(file)


def barplot(
    df,
    labels,
    y_label,
    file,
    palette,
    hue="algorithm",
    figsize=(20, 15),
    sequential_watermark=False,
):
    sns.set_style("whitegrid")
    sns.set_context("talk", font_scale=2.0)
    plt.figure(figsize=figsize)
    ax = sns.barplot(
        data=df,
        x="repository",
        y="y",
        hue=hue,
        # showmeans=True,
        # width=0.75,
        # meanprops={
        #    "marker": "v",
        #    "markerfacecolor": "white",
        #    "markeredgecolor": "black",
        #    "markersize": "8"
        # },
        palette=palette,
    )
    ax.set_xticklabels(labels=labels)
    ax.set_xlabel("")
    ax.set_ylabel(y_label)
    ax.get_yaxis().set_minor_locator(mpl.ticker.AutoMinorLocator())
    ax.grid(which="major", linewidth=1.0)
    ax.grid(which="minor", linewidth=0.5)
    if sequential_watermark:
        plt.figtext(0.01, 0.02, "single-threaded", color="grey", rotation="vertical")
    plt.legend(loc="best")
    plt.tight_layout(pad=0.2)
    plt.savefig(file)


def stripplot(
    df,
    labels,
    y_label,
    file,
    palette=None,
    hue="algorithm",
    figsize=(20, 15),
    legend=True,
    legend_loc="best",
    legend_anchor=None,
    sequential_watermark=False,
):
    sns.set_style("whitegrid")
    sns.set_context("talk", font_scale=2.0)
    plt.figure(figsize=figsize)
    ax = sns.stripplot(
        data=df,
        x="repository",
        y="y",
        hue=hue,
        dodge=True,
        jitter=0.3,
        size=8,
        linewidth=1,
        palette=palette,
    )
    ax.set_xticklabels(labels=labels, rotation="vertical")
    ax.set_xlabel("")
    ax.set_ylabel(y_label)
    ax.get_yaxis().set_minor_locator(mpl.ticker.AutoMinorLocator())
    ax.grid(which="major", linewidth=1.0)
    ax.grid(which="minor", linewidth=0.5)
    if legend:
        plt.legend(loc=legend_loc, bbox_to_anchor=legend_anchor)
    else:
        plt.legend([], [], frameon=False)
    if sequential_watermark:
        plt.figtext(0.01, 0.02, "single-threaded", color="grey", rotation="vertical")
    plt.tight_layout(pad=0.2)
    plt.savefig(file)


def scatterplot(
    df_raw,
    labels,
    x_label,
    y_label,
    file,
    palette=None,
    hue="algorithm",
    figsize=(20, 15),
    x_scale="linear",
    y_scale="linear",
    legend=True,
    legend_loc="best",
    legend_anchor=None,
    regression=False,
    sequential_watermark=False,
):
    df = pd.concat(df_raw)

    sns.set_style("whitegrid")
    sns.set_context("talk", font_scale=2.0)
    plt.figure(figsize=figsize)
    ax = sns.scatterplot(
        data=df,
        x="x",
        y="y",
        hue=hue,
        linewidth=1,
        edgecolor="black",
        palette=palette,
        legend="full",
    )
    if regression:
        for i in range(len(df_raw)):
            ax = sns.regplot(
                data=df_raw[i],
                x="x",
                y="y",
                logx=True,
                label=labels[i],
                scatter=False,
                truncate=False,
                order=1,
                color=palette[i],
            )

    ax.set_xscale(x_scale)
    ax.set_yscale(y_scale)

    ax.set_xlabel(x_label)
    ax.set_ylabel(y_label)
    ax.get_yaxis().set_minor_locator(mpl.ticker.AutoMinorLocator())
    ax.get_xaxis().set_minor_locator(mpl.ticker.AutoMinorLocator())
    ax.grid(which="major", linewidth=1.0)
    ax.grid(which="minor", linewidth=0.5)
    if legend:
        plt.legend(loc=legend_loc, bbox_to_anchor=legend_anchor)
    else:
        plt.legend([], [], frameon=False)
    if sequential_watermark:
        plt.figtext(0.01, 0.02, "single-threaded", color="grey")
    plt.tight_layout(pad=0.2)
    plt.savefig(file)
