import pandas as pd
import seaborn as sns
import matplotlib as mpl
import matplotlib.pyplot as plt

from .labels import get_labels_git, get_labels_mutants


def get_test_diff_and_intersection(retest_all, other):
    retest_all_tests = set(retest_all.splitlines()) if retest_all else set()
    other_tests = set(other.splitlines()) if other else set()
    return list(retest_all_tests.difference(other_tests)), list(
        retest_all_tests.intersection(other_tests)
    )


def get_test_diff(retest_all, other):
    retest_all_tests = retest_all.splitlines()
    other_tests = other.splitlines()
    return list(set(retest_all_tests) - set(other_tests))


class HistoryPlotter:
    def __init__(self, connection, output_format, sequential_watermark=False):
        self.connection = connection
        self.output_format = output_format
        self.sequential_watermark = sequential_watermark

        self.labels = get_labels_git(connection)

    def plot_history_duration_absolute(self):
        y_label = "absolute e2e testing time [s]"
        file = "duration_absolute" + self.output_format

        df_retest_all = self.connection.raw_query(
            """
            SELECT c.repo_id as repository,
                retest_all_test_duration as y,
                'retest-all' as algorithm
            FROM "TestReportExtended" join "Commit" c on c.id = commit ORDER BY commit
            """
        )

        df_dynamic = self.connection.raw_query(
            """
            SELECT c.repo_id as repository,
                dynamic_test_duration as y,
                'dynamic' as algorithm
            FROM "TestReportExtended" join "Commit" c on c.id = commit ORDER BY commit
            """
        )

        df_static = self.connection.raw_query(
            """
            SELECT c.repo_id as repository,
                static_test_duration as y,
                'static' as algorithm
            FROM "TestReportExtended" join "Commit" c on c.id = commit ORDER BY commit
            """
        )

        df = pd.concat([df_retest_all, df_dynamic, df_static])

        boxplot(
            df,
            self.labels,
            y_label,
            file,
            ["#DAD7CB", "#E37222", "#A2AD00"],
            sequential_watermark=self.sequential_watermark,
        )

    def plot_history_duration_relative(self):
        y_label = "relative e2e testing time [%]"
        file = "duration_relative" + self.output_format

        df_dynamic = self.connection.raw_query(
            """
            SELECT c.repo_id as repository,
                CAST(dynamic_test_duration * 100.0 / retest_all_test_duration AS FLOAT) as y,
                'dynamic' as algorithm
            FROM "TestReportExtended" join "Commit" c on c.id = commit ORDER BY commit
            """
        )

        df_static = self.connection.raw_query(
            """
            SELECT c.repo_id as repository,
                CAST(static_test_duration * 100.0 / retest_all_test_duration AS FLOAT) as y,
                'static' as algorithm
            FROM "TestReportExtended" join "Commit" c on c.id = commit ORDER BY commit
            """
        )

        df = pd.concat([df_dynamic, df_static])

        boxplot(
            df,
            self.labels,
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

        df_dynamic = self.connection.raw_query(
            """
            SELECT CAST(retest_all_mean AS FLOAT) as x,
                CAST(dynamic_mean_relative AS FLOAT) as y,
                'dynamic' as algorithm FROM "Duration"
            """
        )

        df_static = self.connection.raw_query(
            """
            SELECT CAST(retest_all_mean AS FLOAT) as x,
                CAST(static_mean_relative AS FLOAT) as y,
                'static' as algorithm FROM "Duration"
            """
        )

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

        df_retest_all_unit = self.connection.raw_query(
            """
            SELECT c.repo_id as repository,
                retest_all_count as y,
                'retest-all - unit' as algorithm
            FROM "TargetCount" join "Commit" c on c.id = commit
            WHERE target = 'UNIT' ORDER BY commit
            """
        )

        df_dynamic_unit = self.connection.raw_query(
            """
            SELECT c.repo_id as repository,
                dynamic_count as y,
                'dynamic - unit' as algorithm
            FROM "TargetCount" join "Commit" c on c.id = commit
            WHERE target = 'UNIT' ORDER BY commit
            """
        )

        df_static_unit = self.connection.raw_query(
            """
            SELECT c.repo_id as repository,
                static_count as y,
                'static - unit' as algorithm
            FROM "TargetCount" join "Commit" c on c.id = commit
            WHERE target = 'UNIT' ORDER BY commit
            """
        )

        df_retest_all_integration = self.connection.raw_query(
            """
            SELECT c.repo_id as repository,
                retest_all_count as y,
                'retest-all - integration' as algorithm
            FROM "TargetCount" join "Commit" c on c.id = commit
            WHERE target = 'INTEGRATION' ORDER BY commit
            """
        )

        df_dynamic_integration = self.connection.raw_query(
            """
            SELECT c.repo_id as repository,
                dynamic_count as y,
                'dynamic - integration' as algorithm
            FROM "TargetCount" join "Commit" c on c.id = commit
            WHERE target = 'INTEGRATION' ORDER BY commit
            """
        )

        df_static_integration = self.connection.raw_query(
            """
            SELECT c.repo_id as repository,
                static_count as y,
                'static - integration' as algorithm
            FROM "TargetCount" join "Commit" c on c.id = commit
            WHERE target = 'INTEGRATION' ORDER BY commit
            """
        )

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
            self.labels,
            y_label,
            file + "_dynamic" + self.output_format,
            ["#E0DED4", "#ADABA1", "#E98C4A", "#B65C1B"],
            sequential_watermark=self.sequential_watermark,
        )
        boxplot(
            df_static,
            self.labels,
            y_label,
            file + "_static" + self.output_format,
            ["#E0DED4", "#ADABA1", "#B4BE26", "#818B00"],
            sequential_watermark=self.sequential_watermark,
        )
        # boxplot(df, labels, y_label, file, ["#DAD7CB", "#E37222", "#A2AD00"])

    def plot_history_target_count_relative(self):
        y_label = "relative number of tests [%]"
        file = "selected_targets_relative"

        df_dynamic_unit = self.connection.raw_query(
            """
            SELECT c.repo_id as repository,
                CAST(dynamic_count * 100.0 / retest_all_count AS FLOAT) as y,
                'dynamic - unit' as algorithm
            FROM "TargetCount" join "Commit" c on c.id = commit
            WHERE target = 'UNIT' ORDER BY commit
            """
        )

        df_static_unit = self.connection.raw_query(
            """
            SELECT c.repo_id as repository,
                CAST(static_count * 100.0 / retest_all_count AS FLOAT) as y,
                'static - unit' as algorithm
            FROM "TargetCount" join "Commit" c on c.id = commit
            WHERE target = 'UNIT' ORDER BY commit
            """
        )

        df_dynamic_integration = self.connection.raw_query(
            """
            SELECT c.repo_id as repository,
                CAST(dynamic_count * 100.0 / retest_all_count AS FLOAT) as y,
                'dynamic - integration' as algorithm
            FROM "TargetCount" join "Commit" c on c.id = commit
            WHERE target = 'INTEGRATION' ORDER BY commit
            """
        )

        df_static_integration = self.connection.raw_query(
            """
            SELECT c.repo_id as repository,
                CAST(static_count * 100.0 / retest_all_count AS FLOAT) as y,
                'static - integration' as algorithm
            FROM "TargetCount" join "Commit" c on c.id = commit
            WHERE target = 'INTEGRATION' ORDER BY commit
            """
        )

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
            self.labels,
            y_label,
            file + self.output_format,
            ["#E98C4A", "#B65C1B", "#B4BE26", "#818B00"],
            sequential_watermark=self.sequential_watermark,
            figsize=(24, 15),
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )
        boxplot(
            df,
            self.labels,
            y_label,
            file + "_boxplot" + self.output_format,
            ["#E98C4A", "#B65C1B", "#B4BE26", "#818B00"],
            sequential_watermark=self.sequential_watermark,
            figsize=(24, 15),
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )
        stripplot(
            df,
            self.labels,
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

        df_selected_dynamic = self.connection.raw_query(
            """
            SELECT c.repo_id as repository,
                commit,
                retest_all,
                dynamic
            FROM "TestCasesSelected" join "Commit" c ON c.id = commit
            ORDER BY commit
            """
        )

        df_selected_static = self.connection.raw_query(
            """
            SELECT c.repo_id as repository,
                commit,
                retest_all,
                static
            FROM "TestCasesSelected" join "Commit" c ON c.id = commit
            ORDER BY commit
            """
        )

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

            labels1 = self.labels[:7] + self.labels[8:]
            labels2 = [self.labels[7]]

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
                self.labels,
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

        df_retest_all = self.connection.raw_query(
            """
            SELECT c.repo_id as repository,
                retest_all_count as y,
                'retest-all' as algorithm
            FROM "TestCasesCount" join "Commit" c on c.id = commit
            ORDER BY commit
            """
        )

        df_dynamic = self.connection.raw_query(
            """
            SELECT c.repo_id as repository,
                dynamic_count as y,
                'dynamic' as algorithm
            FROM "TestCasesCount" join "Commit" c on c.id = commit
            ORDER BY commit
            """
        )

        df_static = self.connection.raw_query(
            """
            SELECT c.repo_id as repository,
                static_count as y,
                'static' as algorithm
            FROM "TestCasesCount" join "Commit" c on c.id = commit
            ORDER BY commit
            """
        )

        df = pd.concat([df_retest_all, df_dynamic, df_static])

        boxplot(
            df,
            self.labels,
            y_label,
            file,
            ["#DAD7CB", "#E37222", "#A2AD00"],
            sequential_watermark=self.sequential_watermark,
        )

    def plot_history_testcases_count_relative(self):
        y_label = "relative number of tests [%]"
        file = "selected_tests_relative"

        df_dynamic = self.connection.raw_query(
            """
            SELECT c.repo_id as repository,
                CAST(dynamic_count * 100.0 / retest_all_count AS FLOAT) as y,
                'dynamic' as algorithm
            FROM "TestCasesCount" join "Commit" c on c.id = commit
            ORDER BY commit
            """
        )

        df_static = self.connection.raw_query(
            """
            SELECT c.repo_id as repository,
                CAST(static_count * 100.0 / retest_all_count AS FLOAT) as y,
                'static' as algorithm
            FROM "TestCasesCount" join "Commit" c on c.id = commit
            ORDER BY commit
            """
        )

        df = pd.concat([df_dynamic, df_static])

        boxplot_with_observations(
            df,
            self.labels,
            y_label,
            file + self.output_format,
            ["#E37222", "#A2AD00"],
            sequential_watermark=self.sequential_watermark,
            figsize=(22, 15),
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )
        boxplot(
            df,
            self.labels,
            y_label,
            file + "_boxplot" + self.output_format,
            ["#E37222", "#A2AD00"],
            sequential_watermark=self.sequential_watermark,
            figsize=(22, 15),
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )
        stripplot(
            df,
            self.labels,
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

        df_different_retest_all = self.connection.raw_query(
            """
            SELECT c.repo_id as repository,
                retest_all,
                commit as commit
            FROM "TestCasesDifferent" join "Commit" c on c.id = commit
            ORDER BY commit
            """
        )

        df_selected_dynamic = self.connection.raw_query(
            """
            SELECT c.repo_id as repository,
                dynamic,
                commit as commit
            FROM "TestCasesSelected" join "Commit" c on c.id = commit
            ORDER BY commit
            """
        )

        df_selected_static = self.connection.raw_query(
            """
            SELECT c.repo_id as repository,
                static,
                commit as commit
            FROM "TestCasesSelected" join "Commit" c on c.id = commit
            ORDER BY commit
            """
        )

        selected_dynamic = []
        not_selected_dynamic = []
        selected_static = []
        not_selected_static = []

        different_retest_all_count = {}

        raw_different_retest_all = df_different_retest_all.to_dict(orient="records")
        raw_selected_dynamic = df_selected_dynamic.to_dict(orient="records")
        raw_selected_static = df_selected_static.to_dict(orient="records")

        print(raw_different_retest_all)

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

        for i in range(len(self.labels)):
            self.labels[i] += (
                "\n("
                + str(different_retest_all_count[i + 1]["count"])
                + " on "
                + str(different_retest_all_count[i + 1]["commits"])
                + ")"
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
            self.labels,
            y_label_selected,
            file_selected,
            ["#E37222", "#A2AD00"],
            hue="algorithm",
            sequential_watermark=self.sequential_watermark,
        )
        stripplot(
            df_not_selected,
            self.labels,
            y_label_not_selected,
            file_not_selected,
            ["#E37222", "#A2AD00"],
            hue="algorithm",
            sequential_watermark=self.sequential_watermark,
        )


########################################################################################################################
# Mutants plots


class MutantsPlotter:
    def __init__(self, connection, output_format):
        super().__init__()
        self.connection = connection
        self.output_format = output_format

        self.labels = get_labels_mutants(connection)

    def plot_mutants_duration_absolute(self):
        y_label = "absolute e2e testing time [s]"
        file = "duration_absolute" + self.output_format

        df_retest_all = self.connection.raw_query(
            """
            SELECT commit as repository,
                retest_all_test_duration as y,
                'retest-all' as algorithm
            FROM "MutantExtended"
            """
        )

        df_dynamic = self.connection.raw_query(
            """
            SELECT commit as repository,
                dynamic_test_duration as y,
                'dynamic' as algorithm
            FROM "MutantExtended"
            """
        )

        df_static = self.connection.raw_query(
            """
            SELECT commit as repository,
                static_test_duration as y,
                'static' as algorithm
            FROM "MutantExtended"
            """
        )

        df = pd.concat([df_retest_all, df_dynamic, df_static])

        boxplot(df, self.labels, y_label, file, ["#DAD7CB", "#E37222", "#A2AD00"])

    def plot_mutants_duration_relative(self):
        y_label = "relative e2e testing time [%]"
        file = "duration_relative" + self.output_format

        df_dynamic = self.connection.raw_query(
            """
            SELECT commit as repository,
                CAST(dynamic_test_duration * 100.0 / retest_all_test_duration AS FLOAT) as y,
                'dynamic' as algorithm
            FROM "MutantExtended"
            """
        )

        df_static = self.connection.raw_query(
            """
            SELECT commit as repository,
                CAST(static_test_duration * 100.0 / retest_all_test_duration AS FLOAT) as y,
                'static' as algorithm
            FROM "MutantExtended"
            """
        )

        df = pd.concat([df_dynamic, df_static])

        boxplot(df, self.labels, y_label, file, ["#E37222", "#A2AD00"])

    def plot_mutants_target_count_absolute(self):
        y_label = "absolute number of tests"
        file = "selected_targets_absolute"

        df_retest_all_unit = self.connection.raw_query(
            """
            SELECT commit as repository,
                retest_all_count as y,
                'retest-all - unit' as algorithm
            FROM "TargetCount"
            WHERE target = 'UNIT'
            """
        )

        df_dynamic_unit = self.connection.raw_query(
            """
            SELECT commit as repository,
                dynamic_count as y,
                'dynamic - unit' as algorithm
            FROM "TargetCount"
            WHERE target = 'UNIT'
            """
        )

        df_static_unit = self.connection.raw_query(
            """
            SELECT commit as repository,
                static_count as y,
                'static - unit' as algorithm
            FROM "TargetCount"
            WHERE target = 'UNIT'
            """
        )

        df_retest_all_integration = self.connection.raw_query(
            """
            SELECT commit as repository,
                retest_all_count as y,
                'retest-all - integration' as algorithm
            FROM "TargetCount"
            WHERE target = 'INTEGRATION'
            """
        )

        df_dynamic_integration = self.connection.raw_query(
            """
            SELECT commit as repository,
                dynamic_count as y,
                'dynamic - integration' as algorithm
            FROM "TargetCount"
            WHERE target = 'INTEGRATION'
            """
        )

        df_static_integration = self.connection.raw_query(
            """
            SELECT commit as repository,
                static_count as y,
                'static - integration' as algorithm
            FROM "TargetCount"
            WHERE target = 'INTEGRATION'
            """
        )

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
            self.labels,
            y_label,
            file + "_dynamic" + self.output_format,
            ["#E0DED4", "#ADABA1", "#E98C4A", "#B65C1B"],
        )
        boxplot(
            df_static,
            self.labels,
            y_label,
            file + "_static" + self.output_format,
            ["#E0DED4", "#ADABA1", "#B4BE26", "#818B00"],
        )

        # boxplot(df, labels, y_label, file, ["#DAD7CB", "#E37222", "#A2AD00"])

    def plot_mutants_target_count_relative(self):
        y_label = "relative number of tests [%]"
        file = "selected_targets_relative"

        df_dynamic_unit = self.connection.raw_query(
            """
            SELECT commit as repository,
                CAST(dynamic_count * 100.0 / retest_all_count AS FLOAT) as y,
                'dynamic - unit' as algorithm
            FROM "TargetCount"
            WHERE target = 'UNIT'
            """
        )

        df_static_unit = self.connection.raw_query(
            """
            SELECT commit as repository,
                CAST(static_count * 100.0 / retest_all_count AS FLOAT) as y,
                'static - unit' as algorithm
            FROM "TargetCount"
            WHERE target = 'UNIT'
            """
        )

        df_dynamic_integration = self.connection.raw_query(
            """
            SELECT commit as repository,
                CAST(dynamic_count * 100.0 / retest_all_count AS FLOAT) as y,
                'dynamic - integration' as algorithm
            FROM "TargetCount"
            WHERE target = 'INTEGRATION'
            """
        )

        df_static_integration = self.connection.raw_query(
            """
            SELECT commit as repository,
                CAST(static_count * 100.0 / retest_all_count AS FLOAT) as y,
                'static - integration' as algorithm
            FROM "TargetCount"
            WHERE target = 'INTEGRATION'
            """
        )

        print(type(df_dynamic_unit["y"][0]))

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
            self.labels,
            y_label,
            file + self.output_format,
            ["#E98C4A", "#B65C1B", "#B4BE26", "#818B00"],
            figsize=(22, 15),
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )
        boxplot(
            df,
            self.labels,
            y_label,
            file + "_boxplot" + self.output_format,
            ["#E98C4A", "#B65C1B", "#B4BE26", "#818B00"],
            figsize=(24, 15),
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )
        stripplot(
            df,
            self.labels,
            y_label,
            file + "_stripplot" + self.output_format,
            ["#E98C4A", "#B65C1B", "#B4BE26", "#818B00"],
            figsize=(24, 15),
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )

    def plot_mutants_testcases_contains_relation(self):
        y_label = "Tests that have been selected"
        file = "contains_all_tests" + self.output_format

        df_selected_dynamic = self.connection.raw_query(
            """
            SELECT commit as repository,
                retest_all_mutant_id,
                dynamic,
                descr as mutant
            FROM "TestCasesSelected"
            WHERE descr != 'baseline'
            ORDER BY commit, descr
            """
        )

        df_selected_static = self.connection.raw_query(
            """
            SELECT commit as repository,
                retest_all_mutant_id,
                static,
                descr as mutant
            FROM "TestCasesSelected"
            WHERE descr != 'baseline'
            ORDER BY commit, descr
            """
        )

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
            self.labels,
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

        df_retest_all = self.connection.raw_query(
            """
            SELECT commit as repository,
                retest_all_count as y,
                'retest-all' as algorithm
            FROM "TestCasesCount"    
            """,
        )

        df_dynamic = self.connection.raw_query(
            """
            SELECT commit as repository,
                dynamic_count as y,
                'dynamic' as algorithm
            FROM "TestCasesCount"
            """
        )

        df_static = self.connection.raw_query(
            """
            SELECT commit as repository,
                static_count as y,
                'static' as algorithm
            FROM "TestCasesCount"
            """
        )

        df = pd.concat([df_retest_all, df_dynamic, df_static])

        boxplot(df, self.labels, y_label, file, ["#DAD7CB", "#E37222", "#A2AD00"])

    def plot_mutants_testcases_count_relative(self):
        y_label = "relative number of tests [%]"
        file = "selected_tests_relative"

        df_dynamic = self.connection.raw_query(
            """
            SELECT commit as repository,
                CAST(dynamic_count * 100.0 / retest_all_count AS FLOAT) as y,
                'dynamic' as algorithm
            FROM "TestCasesCount"
            """
        )

        df_static = self.connection.raw_query(
            """
            SELECT commit as repository,
                CAST(static_count * 100.0 / retest_all_count AS FLOAT) as y,
                'static' as algorithm
            FROM "TestCasesCount"
            """
        )

        df = pd.concat([df_dynamic, df_static])

        boxplot_with_observations(
            df,
            self.labels,
            y_label,
            file + self.output_format,
            ["#E37222", "#A2AD00"],
            figsize=(22, 15),
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )
        boxplot(
            df,
            self.labels,
            y_label,
            file + "_boxplot" + self.output_format,
            ["#E37222", "#A2AD00"],
            figsize=(22, 15),
            legend_anchor=(1.0, 0.8, 0.1, 0.1),
        )
        stripplot(
            df,
            self.labels,
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

        df_failed_retest_all = self.connection.raw_query(
            """
            SELECT commit as repository,
                retest_all_mutant_id,
                retest_all,
                descr as mutant
            FROM "TestCasesFailed"
            WHERE descr != 'baseline'
            ORDER BY commit, descr
            """
        )

        df_selected_dynamic = self.connection.raw_query(
            """
            SELECT commit as repository,
                retest_all_mutant_id,
                dynamic,
                descr as mutant
            FROM "TestCasesSelected"
            WHERE descr != 'baseline'
            ORDER BY commit, descr
            """
        )

        df_selected_static = self.connection.raw_query(
            """
            SELECT commit as repository,
                retest_all_mutant_id,
                static,
                descr as mutant
            FROM "TestCasesSelected"
            WHERE descr != 'baseline'
            ORDER BY commit, descr
            """
        )

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

            # df_selected_1 = df_selected[(df_selected["repository"].isin(filter_normal))]
            # df_selected_2 = df_selected[(df_selected["repository"].isin(filter_special))]
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
                self.labels,
                "",
                file_not_selected + self.output_format,
                ["#E37222", "#A2AD00"],
                hue="algorithm",
                figsize=(17, 15),
                legend=False,
            )

        # stripplot(df_selected_1, labels1, y_label_selected, file_selected + "_1" + output_format, ["#E37222", "#A2AD00"],
        #          hue='algorithm', figsize=(17, 15))
        # stripplot(df_selected_2, labels2, "", file_selected + "_2" + output_format, ["#E37222", "#A2AD00"], hue='algorithm',
        #          figsize=(3, 15),
        #          legend=False)
        stripplot(
            df_selected,
            self.labels,
            y_label_selected,
            file_selected + self.output_format,
            ["#E37222", "#A2AD00"],
            hue="algorithm",
            figsize=(17, 15),
        )

    def plot_mutants_percentage_failed(self):
        y_label = "failed tests of selected tests [%]"
        file = "selected_tests_percentage_failed"

        df_retest_all = self.connection.raw_query(
            """
            SELECT commit as repository,
                CAST(retest_all_count_failed * 100.0 / retest_all_count AS FLOAT) as y,
                'retest-all' as algorithm
            FROM "TestCasesCount"
            WHERE retest_all_count != 0
                and dynamic_count != 0
                and static_count != 0
            """
        )

        df_dynamic = self.connection.raw_query(
            """
            SELECT commit as repository,
                CAST(dynamic_count_failed * 100.0 / dynamic_count AS FLOAT) as y,
                'dynamic' as algorithm
            FROM "TestCasesCount"
            WHERE retest_all_count != 0
                and dynamic_count != 0
                and static_count != 0
            """
        )

        df_static = self.connection.raw_query(
            """
            SELECT commit as repository,
                CAST(static_count_failed * 100.0 / static_count AS FLOAT) as y,
                'static' as algorithm
            FROM "TestCasesCount"
            WHERE retest_all_count != 0
                and dynamic_count != 0
                and static_count != 0
            """
        )

        df = pd.concat([df_retest_all, df_dynamic, df_static])

        boxplot(
            df,
            self.labels,
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
