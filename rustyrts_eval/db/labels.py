import pandas as pd


def get_labels_mutants(connection, count=True):
    df_labels = connection.raw_query(
        """
        SELECT r.path, count(distinct m.descr) as number_mutants
        FROM "Repository" r, "Commit" c, "MutantTestCaseOverview" m
        WHERE r.id = c.repo_id AND c.id = m.commit
        GROUP BY c.id, r.path
        ORDER BY c.id
        """
    )

    labels = []
    for row in df_labels.to_dict(orient="records"):
        if count:
            labels.append(
                row["path"][row["path"].rfind("/") + 1 :]
                + "\n("
                + str(row["number_mutants"])
                + ")"
            )
        else:
            labels.append(row["path"][row["path"].rfind("/") + 1 :])

    padding = max(len(label.splitlines()[0]) for label in labels)
    for i in range(len(labels)):
        lines = []
        first_line = labels[i].splitlines()[0]
        for line in labels[i].splitlines():
            lines.append((padding - len(first_line)) * "  " + line)
        labels[i] = "\n".join(lines)

    return labels


def get_labels_git(connection):
    df_labels = connection.raw_query(
        """
        SELECT r.id, r.path, count(distinct t.commit) as number_commits
        FROM "Repository" r, "Commit" c, "TestCaseOverview" t
        WHERE r.id = c.repo_id AND c.id = t.commit 
        GROUP BY r.id, r.path
        ORDER BY r.id
        """
    )

    labels = []
    for row in df_labels.to_dict(orient="records"):
        labels.append(row["path"][row["path"].rfind("/") + 1 :])

    #    padding = max(len(label.splitlines()[0]) for label in labels)
    #    for i in range(len(labels)):
    #        lines = []
    #        first_line = labels[i].splitlines()[0]
    #        for line in labels[i].splitlines():
    #            lines.append((padding - len(first_line)) * " " + line)
    #        labels[i] = '\n'.join(lines)

    return labels
