import pandas as pd
import seaborn as sns
import matplotlib as mpl
import matplotlib.pyplot as plt
from sqlalchemy.sql import Select, select

from .analysis import get_test_diff, get_test_diff_and_intersection
from .git import DBCommit

COLORS_REGULAR = [
    ["#DAD7CB", "#005293", "#E37222", "#A2AD00"],
    ["#005293", "#E37222", "#A2AD00"],
    [
        ["#0062b3", "#375d84", "#003866", "#E98C4A", "#c77d51", "#B65C1B", "#B4BE26", "#98a24d", "#818B00"],
        ["#E0DED4", "#ADABA1", "#7f7e76", "#0062b3", "#375d84", "#003866"],
        ["#E0DED4", "#ADABA1", "#7f7e76", "#E98C4A", "#c77d51", "#B65C1B"],
        ["#E0DED4", "#ADABA1", "#7f7e76", "#B4BE26", "#98a24d", "#818B00"],
    ],
    ["#E37222"],
]

COLORS_BLIND = [
    ["#4343F9", "#005293", "#BA1414", "#14BA14"],
    ["#005293", "#BA1414", "#14BA14"],
    [
        ["#ffaa00", "#996600", "#D85B5B", "#A51A1A", "#53BC53", "#118411"],
        ["#7E7EfC", "#24249E", "#ffaa00", "#996600"],
        ["#7E7EfC", "#24249E", "#D85B5B", "#A51A1A"],
        ["#7E7EfC", "#24249E", "#53BC53", "#118411"],
    ],
    ["#BA1414"],
]

COLORS = COLORS_REGULAR


class HistoryPlotter:
    def __init__(self, connection, view_info, output_format, sequential_watermark=False):
        self.connection = connection
        self.view_info = view_info
        self.output_format = output_format
        self.sequential_watermark = sequential_watermark

        self.labels = view_info.get_labels(connection)
        self.order_dict: dict[int, int] = {}
        for i, k in enumerate(self.labels.set_index("id")["path"].to_dict(), start=1):
            self.order_dict[k] = i
        self.labels["id"] = self.labels["id"].map(lambda x: self.order_dict[x])

    def query(self, query: Select) -> pd.DataFrame:
        df = self.connection.query(query)
        df["repository"] = df["repository"].map(lambda x: self.order_dict[x])
        return df

    def plot_history_duration_absolute(self, partition=False):
        y_label = "absolute e2e testing time [s]"
        file = "duration_absolute" + self.output_format

        commit = DBCommit.__table__
        testreport_extended = self.view_info.testreport_extended

        durations = (
            select(
                commit.c.repo_id.label("repository"),
                testreport_extended.c.retest_all_test_duration,
                testreport_extended.c.basic_test_duration,
                testreport_extended.c.dynamic_test_duration,
                testreport_extended.c.static_test_duration,
            )
            .select_from(testreport_extended, commit)
            .where(commit.c.id == testreport_extended.c.commit)
            .order_by(testreport_extended.c.commit)
        )

        df = self.query(durations)

        df_retest_all = df[["repository"]].copy()
        df_retest_all["y"] = df["retest_all_test_duration"]
        df_retest_all["algorithm"] = "retest-all"

        df_basic = df[["repository"]].copy()
        df_basic["y"] = df["basic_test_duration"]
        df_basic["algorithm"] = "basic"

        df_dynamic = df[["repository"]].copy()
        df_dynamic["y"] = df["dynamic_test_duration"]
        df_dynamic["algorithm"] = "dynamic"

        df_static = df[["repository"]].copy()
        df_static["y"] = df["static_test_duration"]
        df_static["algorithm"] = "static"

        df = pd.concat([df_retest_all, df_basic, df_dynamic, df_static])
        dfs = [df]
        labels = [self.labels["path"]]

        if partition:
            filter_normal = [1, 2, 4, 5, 6, 8, 9, 10, 11]
            filter_special = [3, 12]
            filter_even_more_special = [7]

            labels_1 = self.labels[(self.labels["id"].isin(filter_normal))]
            labels_2 = self.labels[(self.labels["id"].isin(filter_special))]
            labels_3 = self.labels[(self.labels["id"].isin(filter_even_more_special))]

            df_1 = df[(df["repository"].isin(filter_normal))]
            df_2 = df[(df["repository"].isin(filter_special))]
            df_3 = df[(df["repository"].isin(filter_even_more_special))]

            dfs = [df_1, df_2, df_3]
            labels = [labels_1["path"], labels_2["path"], labels_3["path"]]

        boxplot(
            dfs,
            labels,
            y_label,
            file,
            COLORS[0],
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
                (testreport_extended.c.basic_test_duration * 100.0 / testreport_extended.c.retest_all_test_duration).label("basic_test_duration"),
                (testreport_extended.c.dynamic_test_duration * 100.0 / testreport_extended.c.retest_all_test_duration).label("dynamic_test_duration"),
                (testreport_extended.c.static_test_duration * 100.0 / testreport_extended.c.retest_all_test_duration).label("static_test_duration"),
            )
            .select_from(testreport_extended, commit)
            .where(commit.c.id == testreport_extended.c.commit)
            .order_by(testreport_extended.c.commit)
        )

        df = self.query(durations)

        df_basic = df[["repository"]].copy()
        df_basic["y"] = df["basic_test_duration"]
        df_basic["algorithm"] = "basic"

        df_dynamic = df[["repository"]].copy()
        df_dynamic["y"] = df["dynamic_test_duration"]
        df_dynamic["algorithm"] = "dynamic"

        df_static = df[["repository"]].copy()
        df_static["y"] = df["static_test_duration"]
        df_static["algorithm"] = "static"

        df = pd.concat([df_dynamic, df_basic, df_static])
        dfs = [df]
        labels = [self.labels["path"]]

        boxplot(
            dfs,
            labels,
            y_label,
            file,
            COLORS[1],
            sequential_watermark=self.sequential_watermark,
        )

    def plot_history_efficiency_repo(self):
        reg_label = "linear regression x~log(y)"
        y_label = "average relative e2e testing time [%]"
        x_label = "average absolute testing time (excluding compilation time)\n of retest-all [s]"
        file = "efficiency_repo"

        duration = self.view_info.duration
        statistics = self.view_info.statistics_repository

        efficiency = (
            select(
                duration.c.repo_id.label("repository"),
                (1.0 * statistics.c.avg_test_duration).label("retest_all_mean_testing_time"),
                (1.0 * duration.c.basic_mean_relative).label("basic_mean_relative"),
                (1.0 * duration.c.dynamic_mean_relative).label("dynamic_mean_relative"),
                (1.0 * duration.c.static_mean_relative).label("static_mean_relative"),
            )
            .select_from(duration, statistics)
            .where(duration.c.repo_id == statistics.c.repo_id)
            .where(duration.c.repo_id != None)
        )

        df = self.query(efficiency)

        df_basic = df[["repository"]].copy()
        df_basic["x"] = df["retest_all_mean_testing_time"]
        df_basic["y"] = df["basic_mean_relative"]
        df_basic["algorithm"] = "basic"

        df_dynamic = df[["repository"]].copy()
        df_dynamic["x"] = df["retest_all_mean_testing_time"]
        df_dynamic["y"] = df["dynamic_mean_relative"]
        df_dynamic["algorithm"] = "dynamic"

        df_static = df[["repository"]].copy()
        df_static["x"] = df["retest_all_mean_testing_time"]
        df_static["y"] = df["static_mean_relative"]
        df_static["algorithm"] = "static"

        scatterplot(
            [df_dynamic, df_static],
            ["basic - " + reg_label, "dynamic - " + reg_label, "static - " + reg_label],
            x_label,
            y_label,
            file + self.output_format,
            COLORS[1],
            sequential_watermark=self.sequential_watermark,
        )

        scatterplot(
            [df_dynamic, df_static],
            ["basic - " + reg_label, "dynamic - " + reg_label, "static - " + reg_label],
            x_label,
            y_label,
            file + "_with_regression" + self.output_format,
            COLORS[1],
            regression=True,
            sequential_watermark=self.sequential_watermark,
        )

    def plot_history_target_count_absolute(self, partition=False):
        y_label = "absolute number of tests"
        file = "selected_targets_absolute"

        commit = DBCommit.__table__
        target_count = self.view_info.target_count

        count_unit = (
            select(
                commit.c.repo_id.label("repository"),
                target_count.c.retest_all_count,
                target_count.c.basic_count,
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
                target_count.c.basic_count,
                target_count.c.dynamic_count,
                target_count.c.static_count,
            )
            .select_from(target_count, commit)
            .where(target_count.c.commit == commit.c.id)
            .where(target_count.c.target == "INTEGRATION")
            .order_by(target_count.c.commit)
        )
        count_doc = (
            select(
                commit.c.repo_id.label("repository"),
                target_count.c.retest_all_count,
                target_count.c.basic_count,
                target_count.c.dynamic_count,
                target_count.c.static_count,
            )
            .select_from(target_count, commit)
            .where(target_count.c.commit == commit.c.id)
            .where(target_count.c.target == "DOCTEST")
            .order_by(target_count.c.commit)
        )

        df_unit = self.query(count_unit)
        df_integration = self.query(count_integration)
        df_doc = self.query(count_doc)

        df_retest_all_unit = df_unit[["repository"]].copy()
        df_retest_all_unit["y"] = df_unit["retest_all_count"]
        df_retest_all_unit["algorithm"] = "retest_all - unit"

        df_basic_unit = df_unit[["repository"]].copy()
        df_basic_unit["y"] = df_unit["basic_count"]
        df_basic_unit["algorithm"] = "basic - unit"

        df_dynamic_unit = df_unit[["repository"]].copy()
        df_dynamic_unit["y"] = df_unit["dynamic_count"]
        df_dynamic_unit["algorithm"] = "dynamic - unit"

        df_static_unit = df_unit[["repository"]].copy()
        df_static_unit["y"] = df_unit["static_count"]
        df_static_unit["algorithm"] = "static - unit"

        df_retest_all_integration = df_integration[["repository"]].copy()
        df_retest_all_integration["y"] = df_integration["retest_all_count"]
        df_retest_all_integration["algorithm"] = "retest_all - integration"

        df_basic_integration = df_integration[["repository"]].copy()
        df_basic_integration["y"] = df_integration["basic_count"]
        df_basic_integration["algorithm"] = "basic - integration"

        df_dynamic_integration = df_integration[["repository"]].copy()
        df_dynamic_integration["y"] = df_integration["dynamic_count"]
        df_dynamic_integration["algorithm"] = "dynamic - integration"

        df_static_integration = df_integration[["repository"]].copy()
        df_static_integration["y"] = df_integration["static_count"]
        df_static_integration["algorithm"] = "static - integration"

        df_retest_all_doc = df_doc[["repository"]].copy()
        df_retest_all_doc["y"] = df_doc["retest_all_count"]
        df_retest_all_doc["algorithm"] = "retest_all - doctest"

        df_basic_doc = df_doc[["repository"]].copy()
        df_basic_doc["y"] = df_doc["basic_count"]
        df_basic_doc["algorithm"] = "basic - doctest"

        df_dynamic_doc = df_doc[["repository"]].copy()
        df_dynamic_doc["y"] = df_doc["dynamic_count"]
        df_dynamic_doc["algorithm"] = "dynamic - doctest"

        df_static_doc = df_doc[["repository"]].copy()
        df_static_doc["y"] = df_doc["static_count"]
        df_static_doc["algorithm"] = "static - doctest"

        df_basic = pd.concat(
            [
                df_retest_all_unit,
                df_retest_all_integration,
                df_retest_all_doc,
                df_basic_unit,
                df_basic_integration,
                df_basic_doc,
            ]
        )
        df_dynamic = pd.concat(
            [
                df_retest_all_unit,
                df_retest_all_integration,
                df_retest_all_doc,
                df_dynamic_unit,
                df_dynamic_integration,
                df_dynamic_doc,
            ]
        )
        df_static = pd.concat(
            [
                df_retest_all_unit,
                df_retest_all_integration,
                df_retest_all_doc,
                df_static_unit,
                df_static_integration,
                df_static_doc,
            ]
        )

        dfs_basic = [df_basic]
        dfs_dynamic = [df_dynamic]
        dfs_static = [df_static]
        labels_basic = [self.labels["path"]]
        labels_dynamic = [self.labels["path"]]
        labels_static = [self.labels["path"]]

        if partition:
            filter_normal = [1, 3, 4, 5, 6, 7, 8, 10, 12]
            filter_special = [2, 11]
            filter_even_more_special = [9]

            labels_basic_1 = self.labels[(self.labels["id"].isin(filter_normal))]
            labels_basic_2 = self.labels[(self.labels["id"].isin(filter_special))]
            labels_basic_3 = self.labels[(self.labels["id"].isin(filter_even_more_special))]

            df_basic_1 = df_basic[(df_basic["repository"].isin(filter_normal))]
            df_basic_2 = df_basic[(df_basic["repository"].isin(filter_special))]
            df_basic_3 = df_basic[(df_basic["repository"].isin(filter_even_more_special))]

            dfs_basic = [df_basic_1, df_basic_2, df_basic_3]
            labels_basic = [
                labels_basic_1["path"],
                labels_basic_2["path"],
                labels_basic_3["path"],
            ]

        boxplot(
            dfs_basic,
            labels_basic,
            y_label,
            file + "_basic" + self.output_format,
            COLORS[2][1],
            sequential_watermark=self.sequential_watermark,
        )

        if partition:
            filter_normal = [1, 3, 4, 5, 6, 7, 8, 10, 12]
            filter_special = [2, 11]
            filter_even_more_special = [9]

            labels_dynamic_1 = self.labels[(self.labels["id"].isin(filter_normal))]
            labels_dynamic_2 = self.labels[(self.labels["id"].isin(filter_special))]
            labels_dynamic_3 = self.labels[(self.labels["id"].isin(filter_even_more_special))]

            df_dynamic_1 = df_dynamic[(df_dynamic["repository"].isin(filter_normal))]
            df_dynamic_2 = df_dynamic[(df_dynamic["repository"].isin(filter_special))]
            df_dynamic_3 = df_dynamic[(df_dynamic["repository"].isin(filter_even_more_special))]

            dfs_dynamic = [df_dynamic_1, df_dynamic_2, df_dynamic_3]
            labels_dynamic = [
                labels_dynamic_1["path"],
                labels_dynamic_2["path"],
                labels_dynamic_3["path"],
            ]

        boxplot(
            dfs_dynamic,
            labels_dynamic,
            y_label,
            file + "_dynamic" + self.output_format,
            COLORS[2][2],
            sequential_watermark=self.sequential_watermark,
        )

        if partition:
            filter_normal = [1, 3, 4, 5, 6, 7, 8, 10, 12]
            filter_special = [2, 11]
            filter_even_more_special = [9]

            labels_static_1 = self.labels[(self.labels["id"].isin(filter_normal))]
            labels_static_2 = self.labels[(self.labels["id"].isin(filter_special))]
            labels_static_3 = self.labels[(self.labels["id"].isin(filter_even_more_special))]

            df_static_1 = df_static[(df_static["repository"].isin(filter_normal))]
            df_static_2 = df_static[(df_static["repository"].isin(filter_special))]
            df_static_3 = df_static[(df_static["repository"].isin(filter_even_more_special))]

            dfs_static = [df_static_1, df_static_2, df_static_3]
            labels_static = [
                labels_static_1["path"],
                labels_static_2["path"],
                labels_static_3["path"],
            ]

        boxplot(
            dfs_static,
            labels_static,
            y_label,
            file + "_static" + self.output_format,
            COLORS[2][3],
            sequential_watermark=self.sequential_watermark,
        )

    def plot_history_target_count_relative(self):
        y_label = "relative number of tests [%]"
        file = "selected_targets_relative"

        commit = DBCommit.__table__
        target_count = self.view_info.target_count

        count_unit = (
            select(
                commit.c.repo_id.label("repository"),
                (target_count.c.basic_count * 100.0 / target_count.c.retest_all_count).label("basic_count"),
                (target_count.c.dynamic_count * 100.0 / target_count.c.retest_all_count).label("dynamic_count"),
                (target_count.c.static_count * 100.0 / target_count.c.retest_all_count).label("static_count"),
            )
            .select_from(target_count, commit)
            .where(target_count.c.commit == commit.c.id)
            .where(target_count.c.target == "UNIT")
            .order_by(target_count.c.commit)
        )
        count_integration = (
            select(
                commit.c.repo_id.label("repository"),
                (target_count.c.basic_count * 100.0 / target_count.c.retest_all_count).label("basic_count"),
                (target_count.c.dynamic_count * 100.0 / target_count.c.retest_all_count).label("dynamic_count"),
                (target_count.c.static_count * 100.0 / target_count.c.retest_all_count).label("static_count"),
            )
            .select_from(target_count, commit)
            .where(target_count.c.commit == commit.c.id)
            .where(target_count.c.target == "INTEGRATION")
            .order_by(target_count.c.commit)
        )
        count_doc = (
            select(
                commit.c.repo_id.label("repository"),
                (target_count.c.basic_count * 100.0 / target_count.c.retest_all_count).label("basic_count"),
                (target_count.c.dynamic_count * 100.0 / target_count.c.retest_all_count).label("dynamic_count"),
                (target_count.c.static_count * 100.0 / target_count.c.retest_all_count).label("static_count"),
            )
            .select_from(target_count, commit)
            .where(target_count.c.commit == commit.c.id)
            .where(target_count.c.target == "DOCTEST")
            .order_by(target_count.c.commit)
        )

        df_unit = self.query(count_unit)
        df_integration = self.query(count_integration)
        df_doc = self.query(count_doc)

        df_basic_unit = df_unit[["repository"]].copy()
        df_basic_unit["y"] = df_unit["basic_count"]
        df_basic_unit["algorithm"] = "basic - unit"

        df_dynamic_unit = df_unit[["repository"]].copy()
        df_dynamic_unit["y"] = df_unit["dynamic_count"]
        df_dynamic_unit["algorithm"] = "dynamic - unit"

        df_static_unit = df_unit[["repository"]].copy()
        df_static_unit["y"] = df_unit["static_count"]
        df_static_unit["algorithm"] = "static - unit"

        df_basic_integration = df_integration[["repository"]].copy()
        df_basic_integration["y"] = df_integration["basic_count"]
        df_basic_integration["algorithm"] = "basic - integration"

        df_dynamic_integration = df_integration[["repository"]].copy()
        df_dynamic_integration["y"] = df_integration["dynamic_count"]
        df_dynamic_integration["algorithm"] = "dynamic - integration"

        df_static_integration = df_integration[["repository"]].copy()
        df_static_integration["y"] = df_integration["static_count"]
        df_static_integration["algorithm"] = "static - integration"

        df_basic_doc = df_doc[["repository"]].copy()
        df_basic_doc["y"] = df_doc["basic_count"]
        df_basic_doc["algorithm"] = "basic - doctest"

        df_dynamic_doc = df_doc[["repository"]].copy()
        df_dynamic_doc["y"] = df_doc["dynamic_count"]
        df_dynamic_doc["algorithm"] = "dynamic - doctest"

        df_static_doc = df_doc[["repository"]].copy()
        df_static_doc["y"] = df_doc["static_count"]
        df_static_doc["algorithm"] = "static - doctest"

        df = pd.concat(
            [
                df_basic_unit,
                df_basic_integration,
                df_basic_doc,
                df_dynamic_unit,
                df_dynamic_integration,
                df_dynamic_doc,
                df_static_unit,
                df_static_integration,
                df_static_doc,
            ]
        )
        dfs = [df]
        labels = [self.labels["path"]]

        boxplot_with_observations(
            dfs,
            labels,
            y_label,
            file + self.output_format,
            COLORS[2][0],
            sequential_watermark=self.sequential_watermark,
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )
        boxplot(
            dfs,
            labels,
            y_label,
            file + "_boxplot" + self.output_format,
            COLORS[2][0],
            sequential_watermark=self.sequential_watermark,
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )
        stripplot(
            dfs,
            labels,
            y_label,
            file + "_stripplot" + self.output_format,
            COLORS[2][0],
            sequential_watermark=self.sequential_watermark,
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )

    def plot_history_testcases_contains_relation(self, partition=False):
        y_label = "tests that have been selected"
        file = "contains_all_tests" + self.output_format

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

        df = self.query(selected)

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

        # TODO: do not commit this
        # df["y"][df["repository"] == 5] = 0

        dfs = [df]
        labels = [self.labels["path"]]

        if partition:
            filter_normal = [1, 2, 3, 4, 5, 7, 8, 9, 10, 11, 12]
            filter_special = [6]

            labels_1 = self.labels[(self.labels["id"].isin(filter_normal))]
            labels_2 = self.labels[(self.labels["id"].isin(filter_special))]

            df_1 = df[(df["repository"].isin(filter_normal))]
            df_2 = df[(df["repository"].isin(filter_special))]

            dfs = [df_1, df_2]
            labels = [labels_1["path"], labels_2["path"]]

        stripplot(
            dfs,
            labels,
            y_label,
            file,
            COLORS[3],
            hue="algorithm",
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
                testcases_count.c.basic_count,
                testcases_count.c.dynamic_count,
                testcases_count.c.static_count,
            )
            .select_from(testcases_count, commit)
            .where(commit.c.id == testcases_count.c.commit)
            .order_by(testcases_count.c.commit)
        )

        df = self.query(count)

        df_retest_all = df[["repository"]].copy()
        df_retest_all["y"] = df["retest_all_count"]
        df_retest_all["algorithm"] = "retest-all"

        df_basic = df[["repository"]].copy()
        df_basic["y"] = df["basic_count"]
        df_basic["algorithm"] = "basic"

        df_dynamic = df[["repository"]].copy()
        df_dynamic["y"] = df["dynamic_count"]
        df_dynamic["algorithm"] = "dynamic"

        df_static = df[["repository"]].copy()
        df_static["y"] = df["static_count"]
        df_static["algorithm"] = "static"

        df = pd.concat([df_retest_all, df_basic, df_dynamic, df_static])
        dfs = [df]
        labels = [self.labels["path"]]

        boxplot(
            dfs,
            labels,
            y_label,
            file,
            COLORS[0],
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
                (testcases_count.c.basic_count * 100.0 / testcases_count.c.retest_all_count).label("basic_count"),
                (testcases_count.c.dynamic_count * 100.0 / testcases_count.c.retest_all_count).label("dynamic_count"),
                (testcases_count.c.static_count * 100.0 / testcases_count.c.retest_all_count).label("static_count"),
            )
            .select_from(testcases_count, commit)
            .where(commit.c.id == testcases_count.c.commit)
            .order_by(testcases_count.c.commit)
        )

        df = self.query(count)

        df_basic = df[["repository"]].copy()
        df_basic["y"] = df["basic_count"]
        df_basic["algorithm"] = "basic"

        df_dynamic = df[["repository"]].copy()
        df_dynamic["y"] = df["dynamic_count"]
        df_dynamic["algorithm"] = "dynamic"

        df_static = df[["repository"]].copy()
        df_static["y"] = df["static_count"]
        df_static["algorithm"] = "static"

        df = pd.concat([df_basic, df_dynamic, df_static])
        dfs = [df]
        labels = [self.labels["path"]]

        boxplot_with_observations(
            dfs,
            labels,
            y_label,
            file + self.output_format,
            COLORS[1],
            sequential_watermark=self.sequential_watermark,
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )
        boxplot(
            dfs,
            labels,
            y_label,
            file + "_boxplot" + self.output_format,
            COLORS[1],
            sequential_watermark=self.sequential_watermark,
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )
        stripplot(
            dfs,
            labels,
            y_label,
            file + "_stripplot" + self.output_format,
            COLORS[1],
            sequential_watermark=self.sequential_watermark,
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )

    def plot_history_testcases_different_absolute(self, partition=False):
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
                testcases_selected.c.basic,
                testcases_selected.c.dynamic,
                testcases_selected.c.static,
            )
            .select_from(testcases_selected, commit)
            .where(commit.c.id == testcases_selected.c.commit)
            .order_by(testcases_selected.c.commit)
        )

        df_different_retest_all = self.query(different_retest_all)
        df_selected_rustyrts = self.query(selected)

        df_selected_basic = df_selected_rustyrts[["repository", "commit", "basic"]].copy()
        df_selected_dynamic = df_selected_rustyrts[["repository", "commit", "dynamic"]].copy()
        df_selected_static = df_selected_rustyrts[["repository", "commit", "static"]].copy()

        selected_basic = []
        not_selected_basic = []
        selected_dynamic = []
        not_selected_dynamic = []
        selected_static = []
        not_selected_static = []

        different_retest_all_count = {}

        raw_different_retest_all = df_different_retest_all.to_dict(orient="records")
        raw_selected_basic = df_selected_basic.to_dict(orient="records")
        raw_selected_dynamic = df_selected_dynamic.to_dict(orient="records")
        raw_selected_static = df_selected_static.to_dict(orient="records")

        assert len(raw_different_retest_all) == len(raw_selected_dynamic) and len(raw_different_retest_all) == len(raw_selected_static) and len(raw_different_retest_all) == len(raw_selected_basic)

        for retest_all_report, basic_report, dynamic_report, static_report in zip(raw_different_retest_all, raw_selected_basic, raw_selected_dynamic, raw_selected_static):
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

            (diff_basic, intersection_basic) = get_test_diff_and_intersection(retest_all_report["retest_all"], basic_report["basic"])
            (diff_dynamic, intersection_dynamic) = get_test_diff_and_intersection(retest_all_report["retest_all"], dynamic_report["dynamic"])
            (diff_static, intersection_static) = get_test_diff_and_intersection(retest_all_report["retest_all"], static_report["static"])

            selected_basic.append(
                {
                    "repository": repository,
                    "commit": commit,
                    "algorithm": "basic",
                    "y": len(intersection_basic),
                }
            )
            not_selected_basic.append(
                {
                    "repository": repository,
                    "commit": commit,
                    "algorithm": "basic",
                    "y": len(diff_basic),
                }
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

        df_selected_basic = pd.DataFrame(selected_basic)
        df_selected_dynamic = pd.DataFrame(selected_dynamic)
        df_selected_static = pd.DataFrame(selected_static)

        df_not_selected_basic = pd.DataFrame(not_selected_basic)
        df_not_selected_dynamic = pd.DataFrame(not_selected_dynamic)
        df_not_selected_static = pd.DataFrame(not_selected_static)

        label = []
        for i in range(len(self.labels)):
            label.append(
                self.labels["path"][i]
                + (
                    # "\n(" +
                    " - " + str(different_retest_all_count[i + 1]["count"]) + " on " + str(different_retest_all_count[i + 1]["commits"])
                    # + ")"
                )
            )

        df_selected = pd.concat(
            [
                df_selected_basic[["repository", "algorithm", "y"]],
                df_selected_dynamic[["repository", "algorithm", "y"]],
                df_selected_static[["repository", "algorithm", "y"]],
            ]
        )

        df_not_selected = pd.concat(
            [
                df_not_selected_basic[["repository", "algorithm", "y"]],
                df_not_selected_dynamic[["repository", "algorithm", "y"]],
                df_not_selected_static[["repository", "algorithm", "y"]],
            ]
        )

        dfs_selected = [df_selected]
        dfs_not_selected = [df_not_selected]
        labels_selected = [label]
        labels_not_selected = [label]

        if partition:
            filter_normal = [1, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]
            filter_special = [2]

            labels_1 = label[:1] + label[2:]
            labels_2 = [label[1]]

            df_selected_1 = df_selected[(df_selected["repository"].isin(filter_normal))]
            df_selected_2 = df_selected[(df_selected["repository"].isin(filter_special))]

            dfs_selected = [df_selected_1, df_selected_2]
            labels_selected = [labels_1, labels_2]

        stripplot(
            dfs_selected,
            labels_selected,
            y_label_selected,
            file_selected,
            COLORS[1],
            hue="algorithm",
            sequential_watermark=self.sequential_watermark,
        )

        stripplot(
            dfs_not_selected,
            labels_not_selected,
            y_label_not_selected,
            file_not_selected,
            COLORS[1],
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
        self.order_dict: dict[int, int] = {}
        for i, k in enumerate(self.labels.set_index("id")["path"].to_dict(), start=1):
            self.order_dict[k] = i
        self.labels["id"] = self.labels["id"].map(lambda x: self.order_dict[x])

    def query(self, query: Select) -> pd.DataFrame:
        df = self.connection.query(query)
        df["repository"] = df["repository"].map(lambda x: self.order_dict[x])
        return df

    def plot_mutants_duration_absolute(self):
        y_label = "absolute e2e testing time [s]"
        file = "duration_absolute" + self.output_format

        mutant_extended = self.view_info.mutant_extended

        durations = (
            select(
                mutant_extended.c.commit.label("repository"),
                mutant_extended.c.retest_all_test_duration,
                mutant_extended.c.basic_test_duration,
                mutant_extended.c.dynamic_test_duration,
                mutant_extended.c.static_test_duration,
            )
            .select_from(mutant_extended)
            .order_by(mutant_extended.c.commit)
        )

        df = self.query(durations)

        df_retest_all = df[["repository"]].copy()
        df_retest_all["y"] = df["retest_all_test_duration"]
        df_retest_all["algorithm"] = "retest-all"

        df_basic = df[["repository"]].copy()
        df_basic["y"] = df["basic_test_duration"]
        df_basic["algorithm"] = "basic"

        df_dynamic = df[["repository"]].copy()
        df_dynamic["y"] = df["dynamic_test_duration"]
        df_dynamic["algorithm"] = "dynamic"

        df_static = df[["repository"]].copy()
        df_static["y"] = df["static_test_duration"]
        df_static["algorithm"] = "static"

        df = pd.concat([df_retest_all, df_basic, df_dynamic, df_static])
        dfs = [df]
        labels = [self.labels["path"]]

        boxplot(
            dfs,
            labels,
            y_label,
            file,
            COLORS[0],
        )

    def plot_mutants_duration_relative(self):
        y_label = "relative e2e testing time [%]"
        file = "duration_relative" + self.output_format

        mutant_extended = self.view_info.mutant_extended

        durations = (
            select(
                mutant_extended.c.commit.label("repository"),
                (mutant_extended.c.basic_test_duration * 100.0 / mutant_extended.c.retest_all_test_duration).label("basic_test_duration"),
                (mutant_extended.c.dynamic_test_duration * 100.0 / mutant_extended.c.retest_all_test_duration).label("dynamic_test_duration"),
                (mutant_extended.c.static_test_duration * 100.0 / mutant_extended.c.retest_all_test_duration).label("static_test_duration"),
            )
            .select_from(mutant_extended)
            .order_by(mutant_extended.c.commit)
        )

        df = self.query(durations)

        df_basic = df[["repository"]].copy()
        df_basic["y"] = df["basic_test_duration"]
        df_basic["algorithm"] = "basic"

        df_dynamic = df[["repository"]].copy()
        df_dynamic["y"] = df["dynamic_test_duration"]
        df_dynamic["algorithm"] = "dynamic"

        df_static = df[["repository"]].copy()
        df_static["y"] = df["static_test_duration"]
        df_static["algorithm"] = "static"

        df = pd.concat([df_basic, df_dynamic, df_static])
        dfs = [df]
        labels = [self.labels["path"]]

        boxplot(dfs, labels, y_label, file, COLORS[1])

    def plot_mutants_target_count_absolute(self, partition=False):
        y_label = "absolute number of tests"
        file = "selected_targets_absolute"

        target_count = self.view_info.target_count

        count_unit = (
            select(
                target_count.c.commit.label("repository"),
                target_count.c.retest_all_count,
                target_count.c.basic_count,
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
                target_count.c.basic_count,
                target_count.c.dynamic_count,
                target_count.c.static_count,
            )
            .select_from(target_count)
            .where(target_count.c.target == "INTEGRATION")
            .order_by(target_count.c.commit)
        )
        count_doc = (
            select(
                target_count.c.commit.label("repository"),
                target_count.c.retest_all_count,
                target_count.c.basic_count,
                target_count.c.dynamic_count,
                target_count.c.static_count,
            )
            .select_from(target_count)
            .where(target_count.c.target == "DOCTEST")
            .order_by(target_count.c.commit)
        )

        df_unit = self.query(count_unit)
        df_integration = self.query(count_integration)
        df_doc = self.query(count_doc)

        df_retest_all_unit = df_unit[["repository"]].copy()
        df_retest_all_unit["y"] = df_unit["retest_all_count"]
        df_retest_all_unit["algorithm"] = "retest_all - unit"

        df_basic_unit = df_unit[["repository"]].copy()
        df_basic_unit["y"] = df_unit["basic_count"]
        df_basic_unit["algorithm"] = "basic - unit"

        df_dynamic_unit = df_unit[["repository"]].copy()
        df_dynamic_unit["y"] = df_unit["dynamic_count"]
        df_dynamic_unit["algorithm"] = "dynamic - unit"

        df_static_unit = df_unit[["repository"]].copy()
        df_static_unit["y"] = df_unit["static_count"]
        df_static_unit["algorithm"] = "static - unit"

        df_retest_all_integration = df_integration[["repository"]].copy()
        df_retest_all_integration["y"] = df_integration["retest_all_count"]
        df_retest_all_integration["algorithm"] = "retest_all - integration"

        df_basic_integration = df_integration[["repository"]].copy()
        df_basic_integration["y"] = df_integration["basic_count"]
        df_basic_integration["algorithm"] = "basic - integration"

        df_dynamic_integration = df_integration[["repository"]].copy()
        df_dynamic_integration["y"] = df_integration["dynamic_count"]
        df_dynamic_integration["algorithm"] = "dynamic - integration"

        df_static_integration = df_integration[["repository"]].copy()
        df_static_integration["y"] = df_integration["static_count"]
        df_static_integration["algorithm"] = "static - integration"

        df_retest_all_doc = df_doc[["repository"]].copy()
        df_retest_all_doc["y"] = df_doc["retest_all_count"]
        df_retest_all_doc["algorithm"] = "retest_all - doctest"

        df_basic_doc = df_doc[["repository"]].copy()
        df_basic_doc["y"] = df_doc["basic_count"]
        df_basic_doc["algorithm"] = "basic - doctest"

        df_dynamic_doc = df_doc[["repository"]].copy()
        df_dynamic_doc["y"] = df_doc["dynamic_count"]
        df_dynamic_doc["algorithm"] = "dynamic - doctest"

        df_static_doc = df_doc[["repository"]].copy()
        df_static_doc["y"] = df_doc["static_count"]
        df_static_doc["algorithm"] = "static - doctest"

        df_basic = pd.concat(
            [
                df_retest_all_unit,
                df_retest_all_integration,
                df_retest_all_doc,
                df_basic_unit,
                df_basic_integration,
                df_basic_doc,
            ]
        )
        df_dynamic = pd.concat(
            [
                df_retest_all_unit,
                df_retest_all_integration,
                df_retest_all_doc,
                df_dynamic_unit,
                df_dynamic_integration,
                df_dynamic_doc,
            ]
        )
        df_static = pd.concat(
            [
                df_retest_all_unit,
                df_retest_all_integration,
                df_retest_all_doc,
                df_static_unit,
                df_static_integration,
                df_static_doc,
            ]
        )

        dfs_basic = [df_basic]
        dfs_dynamic = [df_dynamic]
        dfs_static = [df_static]
        labels_basic = [self.labels["path"]]
        labels_dynamic = [self.labels["path"]]
        labels_static = [self.labels["path"]]

        if partition:
            filter_normal = [1, 3, 5, 6, 7, 9]
            filter_special = [2, 8]
            filter_even_more_special = [4]

            labels_basic_1 = self.labels[(self.labels["id"].isin(filter_normal))]
            labels_basic_2 = self.labels[(self.labels["id"].isin(filter_special))]
            labels_basic_3 = self.labels[(self.labels["id"].isin(filter_even_more_special))]

            df_basic_1 = df_basic[(df_basic["repository"].isin(filter_normal))]
            df_basic_2 = df_basic[(df_basic["repository"].isin(filter_special))]
            df_basic_3 = df_basic[(df_basic["repository"].isin(filter_even_more_special))]

            dfs_basic = [df_basic_1, df_basic_2, df_basic_3]
            labels_basic = [
                labels_basic_1["path"],
                labels_basic_2["path"],
                labels_basic_3["path"],
            ]

        boxplot(
            dfs_basic,
            labels_basic,
            y_label,
            file + "_basic" + self.output_format,
            COLORS[2][1],
        )

        if partition:
            filter_normal = [1, 3, 5, 6, 7, 9]
            filter_special = [2, 8]
            filter_even_more_special = [4]

            labels_dynamic_1 = self.labels[(self.labels["id"].isin(filter_normal))]
            labels_dynamic_2 = self.labels[(self.labels["id"].isin(filter_special))]
            labels_dynamic_3 = self.labels[(self.labels["id"].isin(filter_even_more_special))]

            df_dynamic_1 = df_dynamic[(df_dynamic["repository"].isin(filter_normal))]
            df_dynamic_2 = df_dynamic[(df_dynamic["repository"].isin(filter_special))]
            df_dynamic_3 = df_dynamic[(df_dynamic["repository"].isin(filter_even_more_special))]

            dfs_dynamic = [df_dynamic_1, df_dynamic_2, df_dynamic_3]
            labels_dynamic = [
                labels_dynamic_1["path"],
                labels_dynamic_2["path"],
                labels_dynamic_3["path"],
            ]

        boxplot(
            dfs_dynamic,
            labels_dynamic,
            y_label,
            file + "_dynamic" + self.output_format,
            COLORS[2][2],
        )

        if partition:
            filter_normal = [1, 3, 5, 6, 7, 9]
            filter_special = [2, 8]
            filter_even_more_special = [4]

            labels_static_1 = self.labels[(self.labels["id"].isin(filter_normal))]
            labels_static_2 = self.labels[(self.labels["id"].isin(filter_special))]
            labels_static_3 = self.labels[(self.labels["id"].isin(filter_even_more_special))]

            df_static_1 = df_static[(df_static["repository"].isin(filter_normal))]
            df_static_2 = df_static[(df_static["repository"].isin(filter_special))]
            df_static_3 = df_static[(df_static["repository"].isin(filter_even_more_special))]

            dfs_static = [df_static_1, df_static_2, df_static_3]
            labels_static = [
                labels_static_1["path"],
                labels_static_2["path"],
                labels_static_3["path"],
            ]

        boxplot(
            dfs_static,
            labels_static,
            y_label,
            file + "_static" + self.output_format,
            COLORS[2][3],
        )

    def plot_mutants_target_count_relative(self):
        y_label = "relative number of tests [%]"
        file = "selected_targets_relative"

        target_count = self.view_info.target_count

        count_unit = (
            select(
                target_count.c.commit.label("repository"),
                (target_count.c.basic_count * 100.0 / target_count.c.retest_all_count).label("basic_count"),
                (target_count.c.dynamic_count * 100.0 / target_count.c.retest_all_count).label("dynamic_count"),
                (target_count.c.static_count * 100.0 / target_count.c.retest_all_count).label("static_count"),
            )
            .select_from(target_count)
            .where(target_count.c.target == "UNIT")
            .order_by(target_count.c.commit)
        )
        count_integration = (
            select(
                target_count.c.commit.label("repository"),
                (target_count.c.basic_count * 100.0 / target_count.c.retest_all_count).label("basic_count"),
                (target_count.c.dynamic_count * 100.0 / target_count.c.retest_all_count).label("dynamic_count"),
                (target_count.c.static_count * 100.0 / target_count.c.retest_all_count).label("static_count"),
            )
            .select_from(target_count)
            .where(target_count.c.target == "INTEGRATION")
            .order_by(target_count.c.commit)
        )
        count_doc = (
            select(
                target_count.c.commit.label("repository"),
                (target_count.c.basic_count * 100.0 / target_count.c.retest_all_count).label("basic_count"),
                (target_count.c.dynamic_count * 100.0 / target_count.c.retest_all_count).label("dynamic_count"),
                (target_count.c.static_count * 100.0 / target_count.c.retest_all_count).label("static_count"),
            )
            .select_from(target_count)
            .where(target_count.c.target == "DOCTEST")
            .order_by(target_count.c.commit)
        )

        df_unit = self.query(count_unit)
        df_integration = self.query(count_integration)
        df_doc = self.query(count_doc)

        df_basic_unit = df_unit[["repository"]].copy()
        df_basic_unit["y"] = df_unit["basic_count"]
        df_basic_unit["algorithm"] = "basic - unit"

        df_dynamic_unit = df_unit[["repository"]].copy()
        df_dynamic_unit["y"] = df_unit["dynamic_count"]
        df_dynamic_unit["algorithm"] = "dynamic - unit"

        df_static_unit = df_unit[["repository"]].copy()
        df_static_unit["y"] = df_unit["static_count"]
        df_static_unit["algorithm"] = "static - unit"

        df_basic_integration = df_integration[["repository"]].copy()
        df_basic_integration["y"] = df_integration["basic_count"]
        df_basic_integration["algorithm"] = "basic - integration"

        df_dynamic_integration = df_integration[["repository"]].copy()
        df_dynamic_integration["y"] = df_integration["dynamic_count"]
        df_dynamic_integration["algorithm"] = "dynamic - integration"

        df_static_integration = df_integration[["repository"]].copy()
        df_static_integration["y"] = df_integration["static_count"]
        df_static_integration["algorithm"] = "static - integration"

        df_basic_doc = df_doc[["repository"]].copy()
        df_basic_doc["y"] = df_doc["basic_count"]
        df_basic_doc["algorithm"] = "basic - doctest"

        df_dynamic_doc = df_doc[["repository"]].copy()
        df_dynamic_doc["y"] = df_doc["dynamic_count"]
        df_dynamic_doc["algorithm"] = "dynamic - doctest"

        df_static_doc = df_doc[["repository"]].copy()
        df_static_doc["y"] = df_doc["static_count"]
        df_static_doc["algorithm"] = "static - doctest"

        df = pd.concat(
            [
                df_basic_unit,
                df_basic_integration,
                df_basic_doc,
                df_dynamic_unit,
                df_dynamic_integration,
                df_dynamic_doc,
                df_static_unit,
                df_static_integration,
                df_static_doc,
            ]
        )
        dfs = [df]
        labels = [self.labels["path"]]

        boxplot_with_observations(
            dfs,
            labels,
            y_label,
            file + self.output_format,
            COLORS[2][0],
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
            size=4,
            linewidth=0.3,
        )
        boxplot(
            dfs,
            labels,
            y_label,
            file + "_boxplot" + self.output_format,
            COLORS[2][0],
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )
        stripplot(
            dfs,
            labels,
            y_label,
            file + "_stripplot" + self.output_format,
            COLORS[2][0],
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
            size=4,
            linewidth=0.3,
        )

    def plot_mutants_testcases_contains_relation(self, partition=False):
        y_label = "tests that have been selected"
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

        df = self.query(selected)

        df_selected_dynamic = df[["repository", "retest_all_mutant_id", "dynamic", "mutant"]]
        df_selected_static = df[["repository", "retest_all_mutant_id", "static", "mutant"]]

        not_selected_static = []

        selected_dynamic = df_selected_dynamic.to_dict(orient="records")
        selected_static = df_selected_static.to_dict(orient="records")

        assert len(selected_static) == len(selected_static)

        for dynamic_mutant, static_mutant in zip(selected_dynamic, selected_static):
            assert dynamic_mutant["retest_all_mutant_id"] == static_mutant["retest_all_mutant_id"]

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

        # TODO: do not commit this
        # df["y"][df["repository"] == 6] = 0

        dfs = [df]
        labels = [self.labels["path"]]

        if partition:
            filter_normal = [1, 2, 3, 4, 6, 8]
            filter_special = [5, 7, 9]

            labels_1 = self.labels[(self.labels["id"].isin(filter_normal))]
            labels_2 = self.labels[(self.labels["id"].isin(filter_special))]

            df_1 = df[(df["repository"].isin(filter_normal))]
            df_2 = df[(df["repository"].isin(filter_special))]

            dfs = [df_1, df_2]
            labels = [labels_1["path"], labels_2["path"]]

        stripplot(
            dfs,
            labels,
            y_label,
            file,
            COLORS[3],
            hue="algorithm",
            legend_loc="upper left",
        )

    def plot_mutants_testcases_count_absolute(self, partition=False):
        y_label = "absolute number of tests"
        file = "selected_tests_absolute" + self.output_format

        testcases_count = self.view_info.testcases_count

        count = (
            select(
                testcases_count.c.commit.label("repository"),
                testcases_count.c.retest_all_count,
                testcases_count.c.basic_count,
                testcases_count.c.dynamic_count,
                testcases_count.c.static_count,
            )
            .select_from(testcases_count)
            .order_by(testcases_count.c.commit)
        )

        df = self.query(count)

        df_retest_all = df[["repository"]].copy()
        df_retest_all["y"] = df["retest_all_count"]
        df_retest_all["algorithm"] = "retest-all"

        df_basic = df[["repository"]].copy()
        df_basic["y"] = df["basic_count"]
        df_basic["algorithm"] = "basic"

        df_dynamic = df[["repository"]].copy()
        df_dynamic["y"] = df["dynamic_count"]
        df_dynamic["algorithm"] = "dynamic"

        df_static = df[["repository"]].copy()
        df_static["y"] = df["static_count"]
        df_static["algorithm"] = "static"

        df = pd.concat([df_retest_all, df_basic, df_dynamic, df_static])
        dfs = [df]
        labels = [self.labels["path"]]

        if partition:
            filter_normal = [1, 2, 3, 5, 6, 7, 8, 9]
            filter_special = [4]

            labels_1 = self.labels[(self.labels["id"].isin(filter_normal))]
            labels_2 = self.labels[(self.labels["id"].isin(filter_special))]

            df_1 = df[(df["repository"].isin(filter_normal))]
            df_2 = df[(df["repository"].isin(filter_special))]

            dfs = [df_1, df_2]
            labels = [labels_1["path"], labels_2["path"]]

        boxplot(
            dfs,
            labels,
            y_label,
            file,
            COLORS[0],
        )

    def plot_mutants_testcases_count_relative(self):
        y_label = "relative number of tests [%]"
        file = "selected_tests_relative"

        testcases_count = self.view_info.testcases_count

        count = (
            select(
                testcases_count.c.commit.label("repository"),
                (testcases_count.c.basic_count * 100.0 / testcases_count.c.retest_all_count).label("basic_count"),
                (testcases_count.c.dynamic_count * 100.0 / testcases_count.c.retest_all_count).label("dynamic_count"),
                (testcases_count.c.static_count * 100.0 / testcases_count.c.retest_all_count).label("static_count"),
            )
            .select_from(testcases_count)
            .order_by(testcases_count.c.commit)
        )

        df = self.query(count)

        df_basic = df[["repository"]].copy()
        df_basic["y"] = df["basic_count"]
        df_basic["algorithm"] = "basic"

        df_dynamic = df[["repository"]].copy()
        df_dynamic["y"] = df["dynamic_count"]
        df_dynamic["algorithm"] = "dynamic"

        df_static = df[["repository"]].copy()
        df_static["y"] = df["static_count"]
        df_static["algorithm"] = "static"

        df = pd.concat([df_basic, df_dynamic, df_static])
        dfs = [df]
        labels = [self.labels["path"]]

        boxplot_with_observations(
            dfs,
            labels,
            y_label,
            file + self.output_format,
            COLORS[1],
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
            size=4,
            linewidth=0.3,
        )
        boxplot(
            dfs,
            labels,
            y_label,
            file + "_boxplot" + self.output_format,
            COLORS[1],
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )
        stripplot(
            dfs,
            labels,
            y_label,
            file + "_stripplot" + self.output_format,
            COLORS[1],
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
            size=4,
            linewidth=0.3,
        )

    def plot_mutants_testcases_failed_absolute(self, partition=False):
        y_label_selected = "Failed tests, selected"
        file_selected = "failed_and_selected_absolute" + self.output_format

        y_label_not_selected = "Failed tests, not selected"
        file_not_selected = "failed_and_not_selected_absolute" + self.output_format

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
                testcases_selected.c.basic,
                testcases_selected.c.dynamic,
                testcases_selected.c.static,
            )
            .select_from(testcases_selected)
            .where(testcases_selected.c.descr != "baseline")
            .order_by(testcases_selected.c.commit, testcases_selected.c.descr)
        )

        df_failed_retest_all = self.query(failed_retest_all)
        df_selected_rustyrts = self.query(selected)

        df_selected_basic = df_selected_rustyrts[["repository", "retest_all_mutant_id", "basic"]].copy()
        df_selected_dynamic = df_selected_rustyrts[["repository", "retest_all_mutant_id", "dynamic"]].copy()
        df_selected_static = df_selected_rustyrts[["repository", "retest_all_mutant_id", "static"]].copy()

        selected_basic = []
        not_selected_basic = []
        selected_dynamic = []
        not_selected_dynamic = []
        selected_static = []
        not_selected_static = []

        raw_failed_retest_all = df_failed_retest_all.to_dict(orient="records")
        raw_selected_basic = df_selected_basic.to_dict(orient="records")
        raw_selected_dynamic = df_selected_dynamic.to_dict(orient="records")
        raw_selected_static = df_selected_static.to_dict(orient="records")

        assert len(raw_failed_retest_all) == len(raw_selected_dynamic) and len(raw_failed_retest_all) == len(raw_selected_static) and len(raw_failed_retest_all) == len(raw_selected_basic)

        for retest_all_mutant, basic_mutant, dynamic_mutant, static_mutant in zip(raw_failed_retest_all, raw_selected_basic, raw_selected_dynamic, raw_selected_static):
            assert retest_all_mutant["retest_all_mutant_id"] == basic_mutant["retest_all_mutant_id"]
            assert retest_all_mutant["retest_all_mutant_id"] == dynamic_mutant["retest_all_mutant_id"]
            assert retest_all_mutant["retest_all_mutant_id"] == static_mutant["retest_all_mutant_id"]

            repository = retest_all_mutant["repository"]
            descr = retest_all_mutant["mutant"]

            (diff_basic, intersection_basic) = get_test_diff_and_intersection(retest_all_mutant["retest_all"], basic_mutant["basic"])
            (diff_dynamic, intersection_dynamic) = get_test_diff_and_intersection(retest_all_mutant["retest_all"], dynamic_mutant["dynamic"])
            (diff_static, intersection_static) = get_test_diff_and_intersection(retest_all_mutant["retest_all"], static_mutant["static"])

            selected_basic.append(
                {
                    "repository": repository,
                    "mutant": descr,
                    "algorithm": "basic",
                    "y": len(intersection_basic),
                }
            )
            not_selected_basic.append(
                {
                    "repository": repository,
                    "mutant": descr,
                    "algorithm": "basic",
                    "y": len(diff_basic),
                }
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

        df_selected_basic = pd.DataFrame(selected_basic)
        df_not_selected_basic = pd.DataFrame(not_selected_basic)
        df_selected_dynamic = pd.DataFrame(selected_dynamic)
        df_not_selected_dynamic = pd.DataFrame(not_selected_dynamic)
        df_selected_static = pd.DataFrame(selected_static)
        df_not_selected_static = pd.DataFrame(not_selected_static)

        df_selected = pd.concat(
            [
                df_selected_basic[["repository", "algorithm", "y"]],
                df_selected_dynamic[["repository", "algorithm", "y"]],
                df_selected_static[["repository", "algorithm", "y"]],
            ]
        )
        df_not_selected = pd.concat(
            [
                df_not_selected_basic[["repository", "algorithm", "y"]],
                df_not_selected_dynamic[["repository", "algorithm", "y"]],
                df_not_selected_static[["repository", "algorithm", "y"]],
            ]
        )

        dfs_selected = [df_selected]
        dfs_not_selected = [df_not_selected]
        labels_selected = [self.labels["path"]]
        labels_not_selected = [self.labels["path"]]

        if partition:
            filter_normal = [1, 2, 3, 4, 6, 8, 9]
            filter_special = [5, 7]

            labels_1 = self.labels[(self.labels["id"].isin(filter_normal))]
            labels_2 = self.labels[(self.labels["id"].isin(filter_special))]

            df_not_selected_1 = df_not_selected[(df_not_selected["repository"].isin(filter_normal))]
            df_not_selected_2 = df_not_selected[(df_not_selected["repository"].isin(filter_special))]

            dfs_not_selected = [df_not_selected_1, df_not_selected_2]
            labels_not_selected = [labels_1["path"], labels_2["path"]]

        stripplot(
            dfs_not_selected,
            labels_not_selected,
            y_label_not_selected,
            file_not_selected,
            COLORS[1],
            hue="algorithm",
        )

        if partition:
            filter_normal = [1, 3, 5, 6, 7, 9]
            filter_special = [2, 4, 8]

            labels_1 = self.labels[(self.labels["id"].isin(filter_normal))]
            labels_2 = self.labels[(self.labels["id"].isin(filter_special))]

            df_selected_1 = df_selected[(df_selected["repository"].isin(filter_normal))]
            df_selected_2 = df_selected[(df_selected["repository"].isin(filter_special))]

            dfs_selected = [df_selected_1, df_selected_2]
            labels_selected = [labels_1["path"], labels_2["path"]]

        stripplot(
            dfs_selected,
            labels_selected,
            y_label_selected,
            file_selected,
            COLORS[1],
            hue="algorithm",
        )

    def plot_mutants_percentage_failed(self):
        y_label = "failed tests of selected tests [%]"
        file = "selected_tests_percentage_failed"

        testcases_count = self.view_info.testcases_count

        count_retest_all = (
            select(
                testcases_count.c.commit.label("repository"),
                (testcases_count.c.retest_all_count_failed * 100.0 / testcases_count.c.retest_all_count).label("y"),
            )
            .select_from(testcases_count)
            .where(testcases_count.c.retest_all_count != 0)
            .order_by(testcases_count.c.commit)
        )
        count_basic = (
            select(
                testcases_count.c.commit.label("repository"),
                (testcases_count.c.basic_count_failed * 100.0 / testcases_count.c.basic_count).label("y"),
            )
            .select_from(testcases_count)
            .where(testcases_count.c.basic_count != 0)
            .order_by(testcases_count.c.commit)
        )
        count_dynamic = (
            select(
                testcases_count.c.commit.label("repository"),
                (testcases_count.c.dynamic_count_failed * 100.0 / testcases_count.c.dynamic_count).label("y"),
            )
            .select_from(testcases_count)
            .where(testcases_count.c.dynamic_count != 0)
            .order_by(testcases_count.c.commit)
        )
        count_static = (
            select(
                testcases_count.c.commit.label("repository"),
                (testcases_count.c.static_count_failed * 100.0 / testcases_count.c.static_count).label("y"),
            )
            .select_from(testcases_count)
            .where(testcases_count.c.static_count != 0)
            .order_by(testcases_count.c.commit)
        )

        df_retest_all = self.query(count_retest_all)
        df_basic = self.query(count_basic)
        df_dynamic = self.query(count_dynamic)
        df_static = self.query(count_static)

        df_retest_all["algorithm"] = "retest-all"
        df_basic["algorithm"] = "basic"
        df_dynamic["algorithm"] = "dynamic"
        df_static["algorithm"] = "static"

        df = pd.concat([df_retest_all, df_basic, df_dynamic, df_static])
        dfs = [df]
        labels = [self.labels["path"]]

        boxplot(
            dfs,
            labels,
            y_label,
            file + self.output_format,
            COLORS[0],
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )


########################################################################################################################
# Plotting utilities


def __get_widths(labels):
    widths = []
    sum = 0
    for label in labels:
        sum += len(label)
    for label in labels:
        widths.append(len(label) * 1.0 / sum)
    return widths


def boxplot(
    dfs,
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
    fig, axes = plt.subplots(1, len(dfs), figsize=figsize, gridspec_kw={"width_ratios": __get_widths(labels)})
    if len(dfs) <= 1:
        axes = [axes]

    for i, (df, label, ax) in enumerate(zip(dfs, labels, axes)):
        for item in [ax.title, ax.xaxis.label, ax.yaxis.label] + ax.get_xticklabels():
            item.set_fontsize(32)
        for item in ax.get_yticklabels():
            item.set_fontsize(24)

        sns.set_style("whitegrid")
        sns.set_context("talk", font_scale=1.6)
        sns.boxplot(
            ax=ax,
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
        ax.set_xticklabels(labels=label, rotation="vertical")
        ax.set_xlabel("")
        ax.get_yaxis().set_minor_locator(mpl.ticker.AutoMinorLocator())
        ax.grid(which="major", linewidth=1.0)
        ax.grid(which="minor", linewidth=0.5)
        if i == 0:
            ax.set_ylabel(y_label)
        else:
            ax.set_ylabel(None)
        if sequential_watermark and i == 0:
            plt.figtext(
                0.01,
                0.02,
                "single-threaded",
                color="grey",
                rotation="vertical",
                fontsize=24,
            )
        if legend and i == 0:
            ax.legend(title="", loc=legend_loc, bbox_to_anchor=legend_anchor)
        else:
            ax.legend([], [], frameon=False)

    fig.tight_layout(pad=0.2)
    fig.savefig(file)


def boxplot_with_observations(
    dfs,
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
    size=8,
    linewidth=0.5,
):
    fig, axes = plt.subplots(1, len(dfs), figsize=figsize, gridspec_kw={"width_ratios": __get_widths(labels)})
    if len(dfs) <= 1:
        axes = [axes]

    for i, (df, label, ax) in enumerate(zip(dfs, labels, axes)):
        for item in [ax.title, ax.xaxis.label, ax.yaxis.label] + ax.get_xticklabels():
            item.set_fontsize(32)
        for item in ax.get_yticklabels():
            item.set_fontsize(24)

        sns.set_style("whitegrid")
        sns.set_context("talk", font_scale=1.6)
        sns.boxplot(
            ax=ax,
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
            size=size,
            linewidth=linewidth,
            palette=palette,
            legend=False,
        )

        ax.set_xticklabels(labels=label, rotation="vertical")
        ax.set_xlabel("")
        ax.get_yaxis().set_minor_locator(mpl.ticker.AutoMinorLocator())
        ax.grid(which="major", linewidth=1.0)
        ax.grid(which="minor", linewidth=0.5)

        if i == 0:
            ax.set_ylabel(y_label)
        else:
            ax.set_ylabel(None)
        if sequential_watermark and i == 0:
            plt.figtext(
                0.01,
                0.02,
                "single-threaded",
                color="grey",
                rotation="vertical",
                fontsize=24,
            )
        if legend and i == 0:
            ax.legend(title="", loc=legend_loc, bbox_to_anchor=legend_anchor)
        else:
            ax.legend([], [], frameon=False)

    fig.tight_layout(pad=0.2)
    fig.savefig(file)


def stripplot(
    dfs,
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
    size=8,
    linewidth=0.5,
):
    fig, axes = plt.subplots(1, len(dfs), figsize=figsize, gridspec_kw={"width_ratios": __get_widths(labels)})
    if len(dfs) <= 1:
        axes = [axes]

    for i, (df, label, ax) in enumerate(zip(dfs, labels, axes)):
        for item in [ax.title, ax.xaxis.label, ax.yaxis.label] + ax.get_xticklabels():
            item.set_fontsize(32)
        for item in ax.get_yticklabels():
            item.set_fontsize(24)

        sns.set_style("whitegrid")
        sns.set_context("talk", font_scale=1.6)
        sns.stripplot(
            ax=ax,
            data=df,
            x="repository",
            y="y",
            hue=hue,
            dodge=True,
            jitter=0.3,
            size=size,
            linewidth=linewidth,
            palette=palette,
            legend=i == 0,
        )
        ax.set_xticklabels(labels=label, rotation="vertical")
        ax.set_xlabel("")
        ax.get_yaxis().set_minor_locator(mpl.ticker.AutoMinorLocator())
        ax.grid(which="major", linewidth=1.0)
        ax.grid(which="minor", linewidth=0.5)
        if i == 0:
            ax.set_ylabel(y_label)
        else:
            ax.set_ylabel(None)
        if sequential_watermark and i == 0:
            plt.figtext(
                0.01,
                0.02,
                "single-threaded",
                color="grey",
                rotation="vertical",
                fontsize=24,
            )
        if legend and i == 0:
            ax.legend(title="", loc=legend_loc, bbox_to_anchor=legend_anchor)
        else:
            ax.legend([], [], frameon=False)

    fig.tight_layout(pad=0.2)
    fig.savefig(file)


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
    linewidth=1.0,
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
        linewidth=linewidth,
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
        ax.legend(title="", loc=legend_loc, bbox_to_anchor=legend_anchor)
    else:
        ax.legend([], [], frameon=False)
    if sequential_watermark:
        plt.figtext(0.01, 0.02, "single-threaded", color="grey")
    plt.tight_layout(pad=0.2)
    plt.savefig(file)
