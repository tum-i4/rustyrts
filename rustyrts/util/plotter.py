import pandas as pd
import seaborn as sns
import matplotlib as mpl
import matplotlib.pyplot as plt




def boxplot(df, labels, y_label, file, palette=None, hue='algorithm'):
    sns.set_style("whitegrid")
    sns.set_context("talk", font_scale=2.2)
    plt.figure(figsize=(20, 15))
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


#    plt.show()


def barplot(df, labels, y_label, file, palette, hue='algorithm'):
    sns.set_style("whitegrid")
    sns.set_context("talk", font_scale=2.2)
    plt.figure(figsize=(20, 10))
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
