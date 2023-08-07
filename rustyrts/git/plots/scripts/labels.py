import pandas as pd

url_git = 'postgresql://postgres:rustyrts@localhost:5432/git_final'
output_format = ".svg"


def get_labels_git():
    df_labels = pd.read_sql(
        '''
        SELECT r.id, r.path, count(distinct t.commit) as number_commits
        FROM public."Repository" r, public."Commit" c, testcase_overview t
        WHERE r.id = c.repo_id AND c.id = t.commit 
        GROUP BY r.id, r.path
        ORDER BY r.id
        ''',
        url_git)

    labels = []
    for row in df_labels.to_dict(orient='records'):
        labels.append(row['path'][row['path'].rfind('/') + 1:])

    padding = max(len(label.splitlines()[0]) for label in labels)
    for i in range(len(labels)):
        lines = []
        first_line = labels[i].splitlines()[0]
        for line in labels[i].splitlines():
            lines.append((padding - len(first_line)) * "  " + line)
        labels[i] = '\n'.join(lines)

    return labels
