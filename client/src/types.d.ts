export type Event = {
	id: string;
	secret?: string;
};

export type Question = {
	/**
	 * Whether the event's host has hidden this question.
	 */
	hidden: boolean;

	/**
	 * ID in ulid format e.g. "01KDZ9BPJF9G0DBNGPDYNA95VX"
	 */
	qid: string;

	/**
	 * Upvotes count.
	 */
	votes: number;

	/**
	 * When the event's host marked this question as answered.
	 *
	 * This value is in unix time format, e.g.: 1767956971.
	 */
	answered?: number;
};
