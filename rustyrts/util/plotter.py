import seaborn as sns
import matplotlib as mpl
import matplotlib.pyplot as plt


def boxplot(df, labels, y_label, file, palette=None, hue='algorithm', figsize=(20, 15)):
    sns.set_style("whitegrid")
    sns.set_context("talk", font_scale=2.0)
    plt.figure(figsize=figsize)
    ax = sns.boxplot(data=df,
                     x='repository',
                     y='y',
                     hue=hue,
                     showmeans=True,
                     width=0.75,
                     meanprops={
                         "marker": "v",
                         "markerfacecolor": "white",
                         "markeredgecolor": "black",
                         "markersize": "16"
                     },
                     fliersize=14,
                     palette=palette)
    ax.set_xticklabels(labels=labels, rotation='vertical')
    ax.set_xlabel("")
    ax.set_ylabel(y_label)
    ax.get_yaxis().set_minor_locator(mpl.ticker.AutoMinorLocator())
    ax.grid(which='major', linewidth=1.0)
    ax.grid(which='minor', linewidth=0.5)
    plt.legend(loc='best')
    plt.tight_layout(pad=0.2)
    plt.savefig(file)


def boxplot_with_observations(df, labels, y_label, file, palette=None, hue='algorithm', figsize=(20, 15)):
    sns.set_style("whitegrid")
    sns.set_context("talk", font_scale=2.0)
    plt.figure(figsize=figsize)
    ax = sns.boxplot(data=df,
                     x='repository',
                     y='y',
                     hue=hue,
                     showmeans=True,
                     width=0.75,
                     meanprops={
                         "marker": "v",
                         "markerfacecolor": "white",
                         "markeredgecolor": "black",
                         "markersize": "16"
                     },
                     fliersize=14,
                     palette=palette)

    sns.stripplot(ax=ax,
                  data=df,
                  x='repository',
                  y='y',
                  hue=hue,
                  dodge=True,
                  jitter=.3,
                  size=8,
                  linewidth=1,
                  palette=palette)

    ax.set_xticklabels(labels=labels, rotation='vertical')
    ax.set_xlabel("")
    ax.set_ylabel(y_label)
    ax.get_yaxis().set_minor_locator(mpl.ticker.AutoMinorLocator())
    ax.grid(which='major', linewidth=1.0)
    ax.grid(which='minor', linewidth=0.5)
    plt.legend(loc='best')
    plt.tight_layout(pad=0.2)
    plt.savefig(file)


def barplot(df, labels, y_label, file, palette, hue='algorithm', figsize=(20, 15)):
    sns.set_style("whitegrid")
    sns.set_context("talk", font_scale=2.0)
    plt.figure(figsize=figsize)
    ax = sns.barplot(data=df,
                     x='repository',
                     y='y',
                     hue=hue,
                     # showmeans=True,
                     # width=0.75,
                     # meanprops={
                     #    "marker": "v",
                     #    "markerfacecolor": "white",
                     #    "markeredgecolor": "black",
                     #    "markersize": "8"
                     # },
                     palette=palette)
    ax.set_xticklabels(labels=labels)
    ax.set_xlabel("")
    ax.set_ylabel(y_label)
    ax.get_yaxis().set_minor_locator(mpl.ticker.AutoMinorLocator())
    ax.grid(which='major', linewidth=1.0)
    ax.grid(which='minor', linewidth=0.5)
    plt.legend(loc='best')
    plt.tight_layout(pad=0.2)
    plt.savefig(file)


def stripplot(df, labels, y_label, file, palette=None, hue='algorithm', figsize=(20, 15), legend=True,
              legend_loc='best', legend_anchor=None):
    sns.set_style("whitegrid")
    sns.set_context("talk", font_scale=2.0)
    plt.figure(figsize=figsize)
    ax = sns.stripplot(data=df,
                       x='repository',
                       y='y',
                       hue=hue,
                       dodge=True,
                       jitter=.3,
                       size=8,
                       linewidth=1,
                       palette=palette)
    ax.set_xticklabels(labels=labels, rotation='vertical')
    ax.set_xlabel("")
    ax.set_ylabel(y_label)
    ax.get_yaxis().set_minor_locator(mpl.ticker.AutoMinorLocator())
    ax.grid(which='major', linewidth=1.0)
    ax.grid(which='minor', linewidth=0.5)
    if legend:
        if legend_anchor:
            plt.legend(loc=legend_loc, bbox_to_anchor=legend_anchor)
        else:
            plt.legend(loc=legend_loc, bbox_to_anchor=legend_anchor)
    else:
        plt.legend([], [], frameon=False)
    plt.tight_layout(pad=0.2)
    plt.savefig(file)
